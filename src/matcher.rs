#[derive(Debug)]
pub struct Match {
    pub start: usize,
    pub end: usize,
}

pub trait Matcher {
    fn find(&self, input: &str) -> Option<Match>;

    // fn find_with_captures(&self, input: &str) -> (Option<Match>, HashMap<usize, Match>);
}
