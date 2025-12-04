use std::collections::{HashMap, HashSet};

const UNIT: &[u8] = b"UNIT";

/// A Non-deterministic Finite Automata for acceptance evaluation is represented here.
pub struct Acceptor<'a> {
    /// Map of states to the states to which they can transition; states are identified as the
    /// index in the vec; Label each state as a unique, positive integer;
    /// Transitions are mapped from the atom to a target position
    transitions: Vec<HashMap<&'a [u8], Vec<usize>>>,
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

fn re_internal_symbol_match(symbol: &[u8], given: &[u8]) -> bool {
    match symbol {
        b"." => true,
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

                        // This state now needs to be tested for further UNIT collapse
                        states.push(*next);
                    }
                }
            }
        }
    }
    collapsed_states
}

fn re_internal_transition<'a>(
    acceptor: &'a Acceptor,
    curr_states: &HashSet<usize>,
    input_char: &[u8],
) -> HashSet<usize> {
    let mut new_states: HashSet<usize> = HashSet::new();

    for state in curr_states.iter() {
        if let Some(transitions) = acceptor.transitions.get(*state) {
            for (symbol, nexts) in transitions.iter() {
                if re_internal_symbol_match(symbol, input_char) {
                    for next in nexts.iter() {
                        new_states.insert(*next);
                    }
                }
            }
        }
    }

    // handle UNIT transition, as they should not consume a character
    new_states = re_internal_follow_null_transitions(acceptor, new_states);

    new_states
}

fn re_internal_check_acceptance(
    accept_states: &HashSet<usize>,
    curr_states: &HashSet<usize>,
) -> bool {
    accept_states.intersection(&curr_states).count() > 0
}

pub fn re_accept_match<'a>(acceptor: &'a Acceptor, input: &[u8]) -> Option<Match> {
    let mut curr_states = HashSet::from([acceptor.start_state]);
    let curr_start = 0;

    let mut curr_match = InternalMatch::new();
    curr_match.start = curr_start as isize;

    for (index, &input_char) in input[curr_start..].iter().enumerate() {
        let new_states = re_internal_transition(acceptor, &curr_states, &[input_char]);

        if re_internal_check_acceptance(&acceptor.accept_states, &new_states) {
            curr_match.end = (index + 1) as isize;
        }

        curr_states = new_states;

        if curr_states.len() == 0 {
            break;
        }

        // if (new_states.len() == 0) {}
    }

    if curr_match.end >= 0 {
        Some(Match::from_internal(curr_match))
    } else {
        None
    }
}

pub fn re_accept_search<'a>(acceptor: &'a Acceptor, input: &[u8]) -> Option<Match> {
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

pub fn re_accept_find_all<'a>(acceptor: &'a Acceptor, input: &[u8]) -> Vec<Match> {
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

impl<'a> Acceptor<'a> {
    pub fn append(&mut self, other: Acceptor<'a>) {}
}
// pub fn re_accept_find_all(

#[cfg(test)]
mod tests {
    use super::*;

    /// Convert byte literals to &[u8]
    #[test]
    fn test_concat() {
        // pattern: `ab+`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a".as_bytes(), vec![1])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let acceptor2 = Acceptor {
            transitions: vec![
                HashMap::from([("a".as_bytes(), vec![1])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        acceptor.append(acceptor2);
    }

    #[test]
    fn test_find_all() {
        // pattern: `ab+`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a".as_bytes(), vec![1])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let test = b"cabb,abababbbbcdabbb";
        let result = re_accept_find_all(&acceptor, test);

        assert_eq!(5, result.len());
        let mut test_match = &result[0];
        assert_eq!(b"abb", &test[test_match.start..test_match.end]);
        test_match = &result[1];
        assert_eq!(b"ab", &test[test_match.start..test_match.end]);
        test_match = &result[2];
        assert_eq!(b"ab", &test[test_match.start..test_match.end]);
        test_match = &result[3];
        assert_eq!(b"abbbb", &test[test_match.start..test_match.end]);
        test_match = &result[4];
        assert_eq!(b"abbb", &test[test_match.start..test_match.end]);
    }

    #[test]
    fn test_search() {
        // pattern: `ab+`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a".as_bytes(), vec![1])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let test = b"cdabbcd";
        let result = re_accept_search(&acceptor, test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!(b"abb", &test[test_match.start..test_match.end]);
    }

    #[test]
    fn test_simple_match() {
        // pattern: `ab+`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a".as_bytes(), vec![1])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let test = b"abcd";
        let result = re_accept_match(&acceptor, test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!(b"ab", &test[test_match.start..test_match.end]);
    }

    #[test]
    fn test_complex_match() {
        // pattern: `ab+c*d`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a".as_bytes(), vec![1])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
                HashMap::from([("b".as_bytes(), vec![2]), (UNIT, vec![3])]),
                HashMap::from([("c".as_bytes(), vec![3]), ("d".as_bytes(), vec![4])]),
                // HashMap::from([(UNIT, 4)]),
            ],
            start_state: 0,
            accept_states: HashSet::from([4]),
        };

        let test = b"abcd";
        let result = re_accept_match(&acceptor, test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!(b"abcd", &test[test_match.start..test_match.end]);

        let test2 = b"abd";
        let result2 = re_accept_match(&acceptor, test2);

        assert!(result2.is_some());
        let test_match2 = result2.unwrap();
        assert_eq!(b"abd", &test2[test_match2.start..test_match2.end]);
    }

    #[test]
    fn test_miss() {
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a".as_bytes(), vec![1])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let result = re_accept_match(&acceptor, b"cdef");

        assert!(result.is_none());
    }

    #[test]
    fn test_longer_match() {
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a".as_bytes(), vec![1])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
            ],
            start_state: 0,
            accept_states: HashSet::from([2]),
        };

        let test: &[u8] = b"abbbbc";

        let result = re_accept_match(&acceptor, test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!(b"abbbb", &test[test_match.start..test_match.end]);
    }

    #[test]
    fn test_branching() {
        // `ab+c|ab*d?`
        let acceptor = Acceptor {
            transitions: vec![
                HashMap::from([("a".as_bytes(), vec![1, 4])]),
                HashMap::from([("b".as_bytes(), vec![2])]),
                HashMap::from([("b".as_bytes(), vec![2]), ("c".as_bytes(), vec![3])]),
                HashMap::from([]), // TODO: consider Option, since that might prevent memory alloc
                HashMap::from([(UNIT, vec![5])]),
                HashMap::from([
                    (UNIT, vec![6]),
                    ("d".as_bytes(), vec![6]),
                    ("b".as_bytes(), vec![5]),
                ]),
            ],
            start_state: 0,
            accept_states: HashSet::from([3, 6]),
        };

        let test: &[u8] = b"def";

        let result = re_accept_match(&acceptor, test);

        assert!(result.is_none());

        let test1: &[u8] = b"abbc";

        let result1 = re_accept_match(&acceptor, test1);

        assert!(result1.is_some());
        let test_match1 = result1.unwrap();
        assert_eq!(b"abbc", &test1[test_match1.start..test_match1.end]);

        let test2: &[u8] = b"a";

        let result2 = re_accept_match(&acceptor, test2);

        assert!(result2.is_some());
        let test_match2 = result2.unwrap();
        assert_eq!(b"a", &test2[test_match2.start..test_match2.end]);

        let test3: &[u8] = b"ad";

        let result3 = re_accept_match(&acceptor, test3);

        assert!(result3.is_some());
        let test_match3 = result3.unwrap();
        assert_eq!(b"ad", &test3[test_match3.start..test_match3.end]);

        let test4: &[u8] = b"abbd";

        let result4 = re_accept_match(&acceptor, test4);

        assert!(result4.is_some());
        let test_match4 = result4.unwrap();
        assert_eq!(b"abbd", &test4[test_match4.start..test_match4.end]);
    }
}
