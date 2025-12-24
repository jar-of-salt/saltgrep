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
