use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
}

pub trait Matcher {
    fn find(&self, input: &str) -> Option<Match>;

    fn captures(&self, input: &str) -> Option<HashMap<u16, Match>>;
}
