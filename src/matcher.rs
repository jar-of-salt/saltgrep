use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
}

impl Match {
    pub fn substr<'a>(&self, subject: &'a str) -> &'a str {
        &subject[self.start..self.end]
    }

    pub fn shift(&self, shift: usize) -> Match {
        Match {
            start: self.start + shift,
            end: self.end + shift,
        }
    }
}

pub trait Matcher {
    fn find_at(&self, input: &str, at: usize) -> Option<Match>;

    fn find(&self, input: &str) -> Option<Match> {
        self.find_at(input, 0)
    }

    fn captures_at(&self, input: &str, at: usize) -> Option<HashMap<u16, Match>>;

    fn captures(&self, input: &str) -> Option<HashMap<u16, Match>> {
        self.captures_at(input, 0)
    }

    fn try_find_iter_at<F, E>(&self, input: &str, at: usize, mut matched: F) -> Result<(), E>
    where
        F: FnMut(Match) -> Result<bool, E>,
    {
        let mut last_end = at;
        // let mut last_match = None;

        loop {
            let found = match self.find_at(input, last_end) {
                Some(found) => found,
                None => return Ok(()),
            };

            if found.start == found.end {
                // zero-width match, move one space forward
                last_end = found.end + 1;
            } else {
                last_end = found.end;
            }

            match matched(found) {
                Ok(true) => (),
                Ok(false) => return Ok(()),
                Err(err) => return Err(err),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expected_substr() {
        let match_result = Match { start: 3, end: 7 };

        assert_eq!(match_result.substr("abcdefghi"), "defg");
    }
}
