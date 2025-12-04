use std::collections::HashSet;

// NOTE: this forces us to use UTF-8
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Rule {
    Range(u8, u8),
    Null,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Transition {
    Target(usize),
    Accept,
}

/// Predicate version
/// A Non-deterministic Finite Automata for acceptance evaluation is represented here.
#[derive(Debug, PartialEq, Eq)]
pub struct Acceptor {
    /// Each state is a vector of unicode ranges and the state they map to
    transitions: Vec<Vec<(Rule, Transition)>>,
}

impl Acceptor {
    pub fn with_capacity(cap: usize) -> Self {
        let mut transitions = Vec::with_capacity(cap);
        transitions.push(vec![(Rule::Null, Transition::Accept)]);
        Acceptor { transitions }
    }

    pub fn default() -> Self {
        // TODO: make this have a reasonable guess of the size of the NFA
        Acceptor::with_capacity(1_000_000)
    }

    // fn do_cons(root: Self, other: Self) -> Self {}

    pub fn cons(&mut self, mut other: Acceptor) {
        let old_accept_idx = self.transitions.len() - 1;
        self.transitions.pop();
        let shift_target = |mut rules: Vec<(Rule, Transition)>| {
            for idx in 0..rules.len() {
                let mut old_value = rules[idx];
                rules[idx] = if let (rule, Transition::Target(next)) = old_value {
                    (rule, Transition::Target(next + old_accept_idx))
                } else {
                    old_value
                };
            }
            rules
        };

        let new_transitions = other.transitions.into_iter().map(shift_target);

        self.transitions.extend(new_transitions);
    }
}

// At state i, take character j and feed it through a series of predicates,
// whichever predicate(s) evaluate true, the target state for that predicate can

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

fn re_internal_symbol_match(rule: &Rule, given: &u8) -> bool {
    match rule {
        Rule::Range(start, end) => start <= given && given <= end,
        Rule::Null => false, // skip Null bc it will collapse from the previous state
    }
}

fn re_internal_follow_null_transitions(
    acceptor: &Acceptor,
    curr_states: HashSet<usize>,
) -> (HashSet<usize>, bool) {
    let mut collapsed_states = HashSet::<usize>::new();
    let mut states = curr_states.into_iter().collect::<Vec<usize>>();

    let mut accept = false;

    while let Some(state) = states.pop() {
        collapsed_states.insert(state);
        if let Some(transitions) = acceptor.transitions.get(state) {
            for (rule, transition) in transitions.iter() {
                if let Rule::Null = rule {
                    match transition {
                        Transition::Target(next) => {
                            collapsed_states.insert(*next);
                            // This state might collapse further
                            states.push(*next);
                        }
                        Transition::Accept => accept = true,
                    }
                }
            }
        }
    }
    (collapsed_states, accept)
}

fn re_internal_transition(
    acceptor: &Acceptor,
    curr_states: &HashSet<usize>,
    input_char: &u8,
    mut accepted: bool,
) -> (HashSet<usize>, bool, bool) {
    let mut new_states: HashSet<usize> = HashSet::new();

    let mut consumed_a_character = false;

    for state in curr_states.iter() {
        if let Some(transitions) = acceptor.transitions.get(*state) {
            for (rule, transition) in transitions.iter() {
                println!("match {}", re_internal_symbol_match(rule, input_char));
                if re_internal_symbol_match(rule, input_char) {
                    consumed_a_character = true;
                    match transition {
                        Transition::Target(next) => {
                            println!("transition to: {}", next);
                            new_states.insert(*next);
                        }
                        Transition::Accept => {
                            println!("Accepted!");
                            accepted = true;
                        }
                    }
                }
            }
        }
    }

    // handle Null transitions, as they should not consume a character
    let (new_states, accepted_via_null) = re_internal_follow_null_transitions(acceptor, new_states);

    (
        new_states,
        accepted || accepted_via_null,
        consumed_a_character,
    )
}

pub fn re_accept_match(acceptor: &Acceptor, input: &[u8]) -> Option<Match> {
    // start state is always the zeroth state
    let mut curr_states = HashSet::from([0]);
    let curr_start = 0;
    let mut accepted = false;
    let accepted_via_null: bool;
    let mut consumed_a_character: bool;

    let mut curr_match = InternalMatch::new();
    curr_match.start = curr_start as isize;

    (curr_states, accepted_via_null) = re_internal_follow_null_transitions(acceptor, curr_states);

    accepted = accepted || accepted_via_null;

    for (index, &input_char) in input[curr_start..].iter().enumerate() {
        (curr_states, accepted, consumed_a_character) =
            re_internal_transition(acceptor, &curr_states, &input_char, accepted);

        if consumed_a_character && accepted {
            curr_match.end = (index + 1) as isize;
        }

        if curr_states.len() == 0 {
            break;
        }
    }

    if curr_match.end >= 0 {
        Some(Match::from_internal(curr_match))
    } else {
        None
    }
}

pub fn re_accept_search(acceptor: &Acceptor, input: &[u8]) -> Option<Match> {
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

pub fn re_accept_find_all(acceptor: &Acceptor, input: &[u8]) -> Vec<Match> {
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

// impl Acceptor {
//     pub fn append(&mut self, other: &Acceptor) {}
// }
// pub fn re_accept_find_all(

#[cfg(test)]
mod tests {
    use super::*;

    /// Convert byte literals to &[u8]
    // #[test]
    // fn test_concat() {
    //     // pattern: `ab+`
    //     let acceptor = Acceptor {
    //         transitions: vec![
    //             HashMap::from([("a".as_bytes(), vec![1])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //         ],
    //         start_state: 0,
    //         accept_states: HashSet::from([2]),
    //     };

    //     let acceptor2 = Acceptor {
    //         transitions: vec![
    //             HashMap::from([("a".as_bytes(), vec![1])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //         ],
    //         start_state: 0,
    //         accept_states: HashSet::from([2]),
    //     };

    //     acceptor.append(acceptor2);
    // }

    // #[test]
    // fn test_find_all() {
    //     // pattern: `ab+`
    //     // let acceptor = Acceptor {
    //     //     transitions: vec![
    //     //         HashMap::from([("a".as_bytes(), vec![1])]),
    //     //         HashMap::from([("b".as_bytes(), vec![2])]),
    //     //         HashMap::from([("b".as_bytes(), vec![2])]),
    //     //     ],
    //     //     start_state: 0,
    //     //     accept_states: HashSet::from([2]),
    //     // };

    //     let acceptor = Acceptor {
    //         transitions: vec![
    //             vec![(Rule::Null, Transition::Target(1))],
    //             vec![(Rule::Range(b'a' as u8, b'a' as u8), Transition::Target(2))],
    //             vec![(Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3))],
    //             vec![
    //                 (Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3)),
    //                 (Rule::Null, Transition::Accept),
    //             ],
    //         ],
    //     };

    //     let test = b"cabb,abababbbbcdabbb";
    //     let result = re_accept_find_all(&acceptor, test);

    //     assert_eq!(5, result.len());
    //     let mut test_match = &result[0];
    //     assert_eq!(b"abb", &test[test_match.start..test_match.end]);
    //     test_match = &result[1];
    //     assert_eq!(b"ab", &test[test_match.start..test_match.end]);
    //     test_match = &result[2];
    //     assert_eq!(b"ab", &test[test_match.start..test_match.end]);
    //     test_match = &result[3];
    //     assert_eq!(b"abbbb", &test[test_match.start..test_match.end]);
    //     test_match = &result[4];
    //     assert_eq!(b"abbb", &test[test_match.start..test_match.end]);
    // }

    // #[test]
    // fn test_search() {
    //     // pattern: `ab+`
    //     let acceptor = Acceptor {
    //         transitions: vec![
    //             HashMap::from([("a".as_bytes(), vec![1])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //         ],
    //         start_state: 0,
    //         accept_states: HashSet::from([2]),
    //     };

    //     let test = b"cdabbcd";
    //     let result = re_accept_search(&acceptor, test);

    //     assert!(result.is_some());
    //     let test_match = result.unwrap();
    //     assert_eq!(b"abb", &test[test_match.start..test_match.end]);
    // }

    fn cons(mut root: Acceptor, mut other: Acceptor) -> Acceptor {
        let old_accept_idx = root.transitions.len() - 1;
        root.transitions.pop();
        let shift_target = |mut rules: Vec<(Rule, Transition)>| {
            for idx in (0..rules.len()) {
                let mut old_value = rules[idx];
                rules[idx] = if let (rule, Transition::Target(next)) = old_value {
                    (rule, Transition::Target(next + old_accept_idx))
                } else {
                    old_value
                };
            }
            rules
        };

        let new_transitions = other.transitions.into_iter().map(shift_target);

        root.transitions.extend(new_transitions);

        root
    }
    #[test]
    fn test_cons() {
        // pattern: `ab+`
        let mut first = Acceptor {
            transitions: vec![
                vec![(Rule::Null, Transition::Target(1))],
                vec![(Rule::Range(b'a' as u8, b'a' as u8), Transition::Target(2))],
                vec![(Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3))],
                vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3)),
                    (Rule::Null, Transition::Target(4)),
                ],
                vec![(Rule::Null, Transition::Accept)],
            ],
        };

        // pattern: `ab+c*d`
        let mut second = Acceptor {
            transitions: vec![
                vec![(Rule::Null, Transition::Target(1))],
                vec![(Rule::Range(b'a' as u8, b'a' as u8), Transition::Target(2))],
                vec![(Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3))],
                vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3)),
                    (Rule::Null, Transition::Target(4)),
                ],
                vec![
                    (Rule::Range(b'c' as u8, b'c' as u8), Transition::Target(4)),
                    (Rule::Range(b'd' as u8, b'd' as u8), Transition::Target(5)),
                ],
                vec![(Rule::Null, Transition::Accept)],
            ],
        };

        first.cons(second);

        // pattern `ab+ab+c*d`
        let result = Acceptor {
            transitions: vec![
                vec![(Rule::Null, Transition::Target(1))],
                vec![(Rule::Range(b'a' as u8, b'a' as u8), Transition::Target(2))],
                vec![(Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3))],
                vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3)),
                    (Rule::Null, Transition::Target(4)),
                ],
                // vec![(Rule::Null, Transition::Accept)], -> replace accept state with start
                // state; shift all other targets by starting index (4 in this case)
                vec![(Rule::Null, Transition::Target(1 + 4))],
                vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Transition::Target(2 + 4),
                )],
                vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Transition::Target(3 + 4),
                )],
                vec![
                    (
                        Rule::Range(b'b' as u8, b'b' as u8),
                        Transition::Target(3 + 4),
                    ),
                    (Rule::Null, Transition::Target(4 + 4)),
                ],
                vec![
                    (
                        Rule::Range(b'c' as u8, b'c' as u8),
                        Transition::Target(4 + 4),
                    ),
                    (
                        Rule::Range(b'd' as u8, b'd' as u8),
                        Transition::Target(5 + 4),
                    ),
                ],
                vec![(Rule::Null, Transition::Accept)],
            ],
        };

        assert_eq!(first, result);
    }

    #[test]
    fn test_simple_match() {
        // pattern: `ab+`
        let acceptor = Acceptor {
            transitions: vec![
                vec![(Rule::Null, Transition::Target(1))],
                vec![(Rule::Range(b'a' as u8, b'a' as u8), Transition::Target(2))],
                vec![(Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3))],
                vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3)),
                    (Rule::Null, Transition::Target(4)),
                ],
                vec![(Rule::Null, Transition::Accept)],
            ],
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
                vec![(Rule::Null, Transition::Target(1))],
                vec![(Rule::Range(b'a' as u8, b'a' as u8), Transition::Target(2))],
                vec![(Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3))],
                vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Transition::Target(3)),
                    (Rule::Null, Transition::Target(4)),
                ],
                vec![
                    (Rule::Range(b'c' as u8, b'c' as u8), Transition::Target(4)),
                    (Rule::Range(b'd' as u8, b'd' as u8), Transition::Target(5)),
                ],
                vec![(Rule::Null, Transition::Accept)],
            ],
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

    // #[test]
    // fn test_miss() {
    //     let acceptor = Acceptor {
    //         transitions: vec![
    //             HashMap::from([("a".as_bytes(), vec![1])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //         ],
    //         start_state: 0,
    //         accept_states: HashSet::from([2]),
    //     };

    //     let result = re_accept_match(&acceptor, b"cdef");

    //     assert!(result.is_none());
    // }

    // #[test]
    // fn test_longer_match() {
    //     let acceptor = Acceptor {
    //         transitions: vec![
    //             HashMap::from([("a".as_bytes(), vec![1])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //         ],
    //         start_state: 0,
    //         accept_states: HashSet::from([2]),
    //     };

    //     let test: &[u8] = b"abbbbc";

    //     let result = re_accept_match(&acceptor, test);

    //     assert!(result.is_some());
    //     let test_match = result.unwrap();
    //     assert_eq!(b"abbbb", &test[test_match.start..test_match.end]);
    // }

    // #[test]
    // fn test_branching() {
    //     // `ab+c|ab*d?`
    //     let acceptor = Acceptor {
    //         transitions: vec![
    //             HashMap::from([("a".as_bytes(), vec![1, 4])]),
    //             HashMap::from([("b".as_bytes(), vec![2])]),
    //             HashMap::from([("b".as_bytes(), vec![2]), ("c".as_bytes(), vec![3])]),
    //             HashMap::from([]), // TODO: consider Option, since that might prevent memory alloc
    //             HashMap::from([(UNIT, vec![5])]),
    //             HashMap::from([
    //                 (UNIT, vec![6]),
    //                 ("d".as_bytes(), vec![6]),
    //                 ("b".as_bytes(), vec![5]),
    //             ]),
    //         ],
    //         start_state: 0,
    //         accept_states: HashSet::from([3, 6]),
    //     };

    //     let test: &[u8] = b"def";

    //     let result = re_accept_match(&acceptor, test);

    //     assert!(result.is_none());

    //     let test1: &[u8] = b"abbc";

    //     let result1 = re_accept_match(&acceptor, test1);

    //     assert!(result1.is_some());
    //     let test_match1 = result1.unwrap();
    //     assert_eq!(b"abbc", &test1[test_match1.start..test_match1.end]);

    //     let test2: &[u8] = b"a";

    //     let result2 = re_accept_match(&acceptor, test2);

    //     assert!(result2.is_some());
    //     let test_match2 = result2.unwrap();
    //     assert_eq!(b"a", &test2[test_match2.start..test_match2.end]);

    //     let test3: &[u8] = b"ad";

    //     let result3 = re_accept_match(&acceptor, test3);

    //     assert!(result3.is_some());
    //     let test_match3 = result3.unwrap();
    //     assert_eq!(b"ad", &test3[test_match3.start..test_match3.end]);

    //     let test4: &[u8] = b"abbd";

    //     let result4 = re_accept_match(&acceptor, test4);

    //     assert!(result4.is_some());
    //     let test_match4 = result4.unwrap();
    //     assert_eq!(b"abbd", &test4[test_match4.start..test_match4.end]);
    // }
}
