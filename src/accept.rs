use std::collections::{HashMap, HashSet};

const UNIT: &str = "UNIT";

/// A Non-deterministic Finite Automata for acceptance evaluation is represented here.
pub struct Acceptor<'a> {
    /// Map of states to the states to which they can transition; states are identified as the
    /// index in the vec
    transitions: Vec<HashMap<&'a str, Vec<usize>>>,
    /// The start state of the machine
    start_state: usize,
    /// The states that count as completed
    accept_states: HashSet<usize>,
    // deterministic: bool;
}

#[derive(Debug)]
struct InternalMatch {
    start: isize,
    end: isize,
}

impl InternalMatch {
    fn set_start(&mut self, start: isize) {
        self.start = start;
    }

    fn set_end(&mut self, end: isize) {
        self.end = end;
    }

    fn new() -> Self {
        InternalMatch { start: -1, end: -1 }
    }
}

#[derive(Debug)]
pub struct Match {
    pub start: usize,
    pub end: usize,
}

impl Match {
    fn from_internal(internal_match: InternalMatch) -> Self {
        Match {
            start: (internal_match.start as usize).try_into().unwrap(),
            end: (internal_match.end as usize).try_into().unwrap(),
        }
    }
}

/// Convert Automaton to Determinite
/// TODO: implement this
// pub fn convert_to_determinite(automaton: FiniteAutomaton) -> FiniteAutomaton {
// }

fn re_internal_symbol_match(symbol: &str, given: &str) -> bool {
    match symbol {
        "." => true,
        UNIT => false, // skip UNIT bc it will collapse from the previous state
        to_match => to_match == given,
    }
}

fn re_internal_follow_null_transitions<'a>(
    acceptor: &'a Acceptor,
    new_states: HashSet<usize>,
) -> HashSet<usize> {
    let mut collapsed_states = HashSet::<usize>::new();
    let mut states = new_states.into_iter().collect::<Vec<usize>>();

    while let Some(state) = states.pop() {
        collapsed_states.insert(state);
        if let Some(transitions) = acceptor.transitions.get(state) {
            for (symbol, nexts) in transitions.iter() {
                if UNIT == *symbol {
                    for next in nexts.iter() {
                        collapsed_states.insert(*next);
                    }
                }
            }
        }
    }
    collapsed_states
}

fn re_internal_check_acceptance(
    accept_states: &HashSet<usize>,
    curr_states: &HashSet<usize>,
) -> bool {
    accept_states.intersection(&curr_states).count() > 0
}

pub fn re_accept_match<'a>(acceptor: &'a Acceptor, input: &str) -> Option<Match> {
    let mut curr_states = HashSet::from([acceptor.start_state]);
    let curr_start = 0;

    let inputs = &input.split_terminator("").collect::<Vec<&str>>()[1..];
    let mut curr_match = InternalMatch::new();
    curr_match.set_start(curr_start as isize);

    for (index, &input) in inputs[curr_start..].iter().enumerate() {
        let mut new_states: HashSet<usize> = HashSet::new();

        for state in curr_states.iter() {
            for (symbol, nexts) in acceptor.transitions[*state].iter() {
                if re_internal_symbol_match(symbol, input) {
                    for next in nexts.iter() {
                        new_states.insert(*next);
                    }
                }
            }
        }

        // handle UNIT transitions, as they should not consume a character
        new_states = re_internal_follow_null_transitions(acceptor, new_states);

        if re_internal_check_acceptance(&acceptor.accept_states, &new_states) {
            curr_match.set_end((index + 1) as isize);
        }

        curr_states = new_states;

        if curr_states.len() == 0 {
            break;
        }

        // if (new_states.len() == 0) {}
    }

    // curr_states = HashSet::from([acceptor.start_state]);

    if curr_match.end >= 0 {
        Some(Match::from_internal(curr_match))
    } else {
        None
    }
}

pub fn re_accept_search<'a>(acceptor: &'a Acceptor, input: &str) -> Option<Match> {
    let mut accept: Option<Match> = None;
    let mut curr_index = 0;

    while let None = accept {
        if curr_index < input.len() {
            accept = re_accept_match(acceptor, &input[curr_index..]);
            if let Some(mut accept_match) = accept {
                accept_match.start = accept_match.start + curr_index;
                accept_match.end = accept_match.end + curr_index;
                accept = Some(accept_match);
            }
            curr_index += 1;
        } else {
            break;
        }
    }

    accept
}

pub fn re_accept_find_all<'a>(acceptor: &'a Acceptor, input: &str) -> Vec<Match> {
    let mut curr_index = 0;
    let mut results = Vec::<Match>::new();

    while let Some(mut accept) = re_accept_search(acceptor, &input[curr_index..]) {
        accept.start += curr_index;
        curr_index += accept.end;
        accept.end = curr_index;
        results.push(accept);
    }

    results
}

// pub fn re_accept_find_all(

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_all() {
        // pattern: `ab+`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a", vec![1])]),
                HashMap::from([("b", vec![2])]),
                HashMap::from([("b", vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let test = "cabb,abababbbbcdabbb";
        let result = re_accept_find_all(&acceptor, test);

        assert_eq!(5, result.len());
        let mut test_match = &result[0];
        assert_eq!("abb", &test[test_match.start..test_match.end]);
        test_match = &result[1];
        assert_eq!("ab", &test[test_match.start..test_match.end]);
        test_match = &result[2];
        assert_eq!("ab", &test[test_match.start..test_match.end]);
        test_match = &result[3];
        assert_eq!("abbbb", &test[test_match.start..test_match.end]);
        test_match = &result[4];
        assert_eq!("abbb", &test[test_match.start..test_match.end]);
    }

    #[test]
    fn test_search() {
        // pattern: `ab+`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a", vec![1])]),
                HashMap::from([("b", vec![2])]),
                HashMap::from([("b", vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let test = "cdabbcd";
        let result = re_accept_search(&acceptor, test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!("abb", &test[test_match.start..test_match.end]);
    }

    #[test]
    fn test_simple_match() {
        // pattern: `ab+`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a", vec![1])]),
                HashMap::from([("b", vec![2])]),
                HashMap::from([("b", vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let test = "abcd";
        let result = re_accept_match(&acceptor, test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!("ab", &test[test_match.start..test_match.end]);
    }

    #[test]
    fn test_complex_match() {
        // pattern: `ab+c*d`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a", vec![1])]),
                HashMap::from([("b", vec![2])]),
                HashMap::from([("b", vec![2]), (UNIT, vec![3])]),
                HashMap::from([("c", vec![3]), ("d", vec![4])]),
                // HashMap::from([(UNIT, 4)]),
            ],
            start_state: 0,
            accept_states: HashSet::from([4]),
        };

        let test = "abcd";
        let result = re_accept_match(&acceptor, test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!("abcd", &test[test_match.start..test_match.end]);

        let test2 = "abd";
        let result2 = re_accept_match(&acceptor, test2);

        assert!(result2.is_some());
        let test_match2 = result2.unwrap();
        assert_eq!("abd", &test2[test_match2.start..test_match2.end]);
    }

    #[test]
    fn test_miss() {
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a", vec![1])]),
                HashMap::from([("b", vec![2])]),
                HashMap::from([("b", vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let result = re_accept_match(&acceptor, "cdef");

        assert!(result.is_none());
    }

    #[test]
    fn test_longer_match() {
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a", vec![1])]),
                HashMap::from([("b", vec![2])]),
                HashMap::from([("b", vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let test: &str = "abbbbc";

        let result = re_accept_match(&acceptor, test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!("abbbb", &test[test_match.start..test_match.end]);
    }

    // #[test]
    // fn test_compound_match() {
    //     /// Perform search for `ab+`
    //     let acceptor = Acceptor {
    //         transitions: vec![
    //             HashMap::from([("a", 1)]),
    //             HashMap::from([("b", 2)]),
    //             HashMap::from([("b", 2)]),
    //         ],
    //         start_state: 0,
    //         accept_states: HashSet::from([2]),
    //     };

    //     let matches = re_accept_match(&acceptor, "abbab");

    //     assert_eq!(matches.len(), 2);
    //     let mut test_match = &matches[0];
    //     assert_eq!(test_match.start, 0);
    //     assert_eq!(test_match.end, 2);
    //     let test_match = &matches[1];
    //     assert_eq!(test_match.start, 3);
    //     assert_eq!(test_match.end, 4);
    // }
}
