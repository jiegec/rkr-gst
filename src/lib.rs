use adler32::RollingAdler32;
use bitvec::prelude::*;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Match {
    pub pattern_index: usize,
    pub text_index: usize,
    pub length: usize,
}

struct RkrGst<'a> {
    pattern: &'a [u8],
    text: &'a [u8],
    pattern_mark: BitVec,
    text_mark: BitVec,
    matches: Vec<Match>,
    result: Vec<Match>,
}

impl<'a> RkrGst<'a> {
    fn scan_pattern(&mut self, search_length: usize) -> usize {
        // map text hashes => text index
        let mut map: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut i = 0;
        while (i + search_length) <= self.text.len() {
            // jump to first unmarked token
            for j in i..(i + search_length) {
                if self.text_mark[j] {
                    i = j + 1;
                    break;
                }
            }
            if i + search_length > self.text.len() {
                break;
            }

            // text[i..i+search_length] is unmarked
            let mut hash = RollingAdler32::new();
            for j in i..(i + search_length) {
                hash.update(self.text[j]);
            }

            // advance until next marked
            loop {
                if self.text_mark[i + search_length - 1] {
                    break;
                }
                map.entry(hash.hash()).or_insert_with(Vec::new).push(i);
                i += 1;
                if i + search_length > self.text.len() {
                    break;
                }
                hash.remove(search_length, self.text[i - 1]);
                hash.update(self.text[i + search_length - 1]);
            }
        }

        // search patterns
        self.matches.clear();
        let mut max_match = 0;
        i = 0;
        while (i + search_length) <= self.pattern.len() {
            // jump to first unmarked token
            for j in i..(i + search_length) {
                if self.pattern_mark[j] {
                    i = j + 1;
                    break;
                }
            }
            if i + search_length > self.pattern.len() {
                break;
            }

            // pattern[i..i+search_length] is unmarked
            let mut hash = RollingAdler32::new();
            for j in i..(i + search_length) {
                hash.update(self.pattern[j]);
            }

            // advance until next marked
            loop {
                if self.pattern_mark[i + search_length - 1] {
                    break;
                }
                if map.contains_key(&hash.hash()) {
                    // found a match, check that it really matches
                    // and try to extend
                    for text_index in &map[&hash.hash()] {
                        let pattern_index = i;
                        let mut k = 0;
                        while *text_index + k < self.text.len()
                            && pattern_index + k < self.pattern.len()
                            && self.text[text_index + k] == self.pattern[pattern_index + k]
                            && !self.text_mark[text_index + k]
                            && !self.pattern_mark[pattern_index + k]
                        {
                            k += 1;
                        }

                        if k > 2 * search_length {
                            return k;
                        }

                        if k >= search_length {
                            self.matches.push(Match {
                                pattern_index,
                                text_index: *text_index,
                                length: k,
                            });
                            max_match = std::cmp::max(max_match, k);
                        }
                    }
                }

                i += 1;
                if i + search_length > self.pattern.len() {
                    break;
                }
                hash.remove(search_length, self.pattern[i - 1]);
                hash.update(self.pattern[i + search_length - 1]);
            }
        }

        max_match
    }

    fn mark_strings(&mut self) {
        // sort by length, desc
        self.matches.sort_by(|a, b| b.length.cmp(&a.length));
        for m in &self.matches {
            let mut unmarked = true;
            for i in 0..m.length {
                if self.text_mark[m.text_index + i] || self.pattern_mark[m.pattern_index + i] {
                    unmarked = false;
                    break;
                }
            }

            if unmarked {
                self.result.push(*m);
                for i in 0..m.length {
                    self.text_mark.set(m.text_index + i, true);
                    self.pattern_mark.set(m.pattern_index + i, true);
                }
            }
        }
        self.matches.clear();
    }
}

pub fn run(
    pattern: &[u8],
    text: &[u8],
    initial_search_length: usize,
    minimum_match_length: usize,
) -> Vec<Match> {
    let mut s = initial_search_length;
    let mut params = RkrGst {
        pattern,
        text,
        pattern_mark: bitvec![0; pattern.len()],
        text_mark: bitvec![0; text.len()],
        matches: vec![],
        result: vec![],
    };
    loop {
        // Lmax := scanpatterns(s)
        let lmax = params.scan_pattern(s);
        // if Lmax > 2 x s
        if lmax > 2 * s {
            // then s := Lmax
            s = lmax;
        } else {
            // markarrays(s)
            params.mark_strings();
            // if s > 2 x minimum_match_length
            if s > 2 * minimum_match_length {
                // s := s div 2
                s /= 2;
            } else if s > minimum_match_length {
                // else if s > minimum_match_length
                // s := minimum_match_length
                s = minimum_match_length;
            } else {
                // stop := true
                break;
            }
        }
    }

    params.result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_match() {
        assert_eq!(
            run("lower".as_bytes(), "yellow".as_bytes(), 3, 2),
            vec![Match {
                pattern_index: 0,
                text_index: 3,
                length: 3
            }]
        );
    }

    #[test]
    fn duplicate_match() {
        assert_eq!(
            run("lowerlow".as_bytes(), "yellow lowlow".as_bytes(), 3, 2),
            vec![
                Match {
                    pattern_index: 0,
                    text_index: 3,
                    length: 3
                },
                Match {
                    pattern_index: 5,
                    text_index: 7,
                    length: 3
                }
            ]
        );
    }
}
