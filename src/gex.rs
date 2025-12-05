use std::collections::HashSet;

// NOTE: this forces us to use UTF-8
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Rule {
    Range(u8, u8),
    Null,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Next {
    Target(usize),
    Accept,
}

pub type Transition = (Rule, Next);

/// Extensible state struct.
/// Goal is to add ability to mark states as the beginning of a group etc, right now just a dumb
/// state machine state.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct State {
    transitions: Vec<Transition>,
}

impl State {
    pub fn from_transitions(transitions: Vec<Transition>) -> Self {
        State { transitions }
    }

    pub fn push(&mut self, transition: Transition) {
        self.transitions.push(transition)
    }
}

/// A Non-deterministic Finite Automata for acceptance evaluation is represented here.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GexMachine {
    /// Each state is a vector of unicode ranges and the state they map to
    states: Vec<State>,
}

fn shifter(shift: usize) -> impl Fn(State) -> State {
    move |mut state: State| {
        for idx in 0..state.transitions.len() {
            let old_value = state.transitions[idx];
            state.transitions[idx] = if let (rule, Next::Target(next)) = old_value {
                (rule, Next::Target(next + shift))
            } else {
                old_value
            };
        }
        state
    }
}

// TODO: implement find w/ explain -> might be hard with this implementation

/// NFA implementation for solving regex.
/// Supports operations to build composite machines via concatenation and alternation.
impl GexMachine {
    /// Create NFA with the given states vec capacity.
    /// The consuming regex matcher should make a best-guess at the eventual size of the NFA to
    /// avoid excessive reallocation.
    pub fn with_capacity(cap: usize) -> Self {
        let mut states = Vec::with_capacity(cap);
        states.push(State {
            transitions: vec![(Rule::Null, Next::Accept)],
        });
        GexMachine { states }
    }

    pub fn default() -> Self {
        // TODO: make this have a reasonable guess of the size of the NFA
        GexMachine::with_capacity(1_000_000)
    }

    // fn do_cons(root: Self, other: Self) -> Self {}

    /// Concatenate the current NFA with another.
    /// The other NFA will be appended to the receiver.
    pub fn cons(mut self, other: GexMachine) -> GexMachine {
        let old_accept_idx = self.states.len() - 1;
        // IMPORTANT Assumption: the last state always contains a singular Accept
        self.states.pop();

        let new_states = other.states.into_iter().map(shifter(old_accept_idx));

        self.states.extend(new_states);

        self
    }

    // TODO: consider allowing arbitrary final states, not just a singular accept?

    /// Alternate the current NFA with another.
    /// The other NFA will be added as the "right hand side" entry of the alternation,
    /// and the receiver will be the "left hand side."
    pub fn or(mut self, other: GexMachine) -> GexMachine {
        self.states.reserve(other.states.len());

        let other_start = self.states.len();

        self.states[0].push((Rule::Null, Next::Target(other_start)));

        let new_accept_idx = self.states.len() + other.states.len() - 1;

        let old_accept = self
            .states
            .last_mut()
            .expect("A non-empty set of states is required")
            .transitions
            .last_mut()
            .expect("An Accept state is required");

        old_accept.1 = Next::Target(new_accept_idx);

        let new_states = other.states.into_iter().map(shifter(other_start));

        self.states.extend(new_states);

        self
    }

    /// Evaluate whether a given input matches the given rule.
    ///
    /// Null transition rules will always evaluate as falsy since they need to be collapsed to next
    /// states without consuming a character, and this is handled separately.
    fn evaluate_rule(rule: &Rule, given: &u8) -> bool {
        match rule {
            Rule::Range(start, end) => start <= given && given <= end,
            Rule::Null => false, // skip Null bc it will collapse from the previous state
        }
    }

    /// Follows Null (Epsilon) transitions until the current states are all non-Null transitions.
    ///
    /// Prevents consumption of input on Null transitions.
    fn collapse_null_transitions(&self, curr_states: HashSet<usize>) -> (HashSet<usize>, bool) {
        // Keep track of visited states to prevent uncontrolled recursive collapse.
        let mut visited = HashSet::<usize>::new();
        let mut collapsed_states = HashSet::<usize>::new();
        let mut states = curr_states.into_iter().collect::<Vec<usize>>();

        let mut accept = false;

        while let Some(last_state) = states.pop() {
            collapsed_states.insert(last_state);

            if visited.contains(&last_state) {
                continue;
            }
            visited.insert(last_state);

            if let Some(state) = self.states.get(last_state) {
                for (rule, transition) in state.transitions.iter() {
                    if let Rule::Null = rule {
                        match transition {
                            Next::Target(next) => {
                                collapsed_states.insert(*next);
                                // This state might collapse further
                                states.push(*next);
                            }
                            Next::Accept => accept = true,
                        }
                    }
                }
            }
        }
        (collapsed_states, accept)
    }

    /// Consumes an input and determines the set of states after the transition.
    fn do_transition(
        &self,
        curr_states: &HashSet<usize>,
        input_char: &u8,
        mut accepted: bool,
    ) -> (HashSet<usize>, bool, bool) {
        let mut new_states: HashSet<usize> = HashSet::new();

        let mut consumed_a_character = false;

        for state_label in curr_states.iter() {
            if let Some(state) = self.states.get(*state_label) {
                for (rule, transition) in state.transitions.iter() {
                    if GexMachine::evaluate_rule(rule, input_char) {
                        consumed_a_character = true;
                        match transition {
                            Next::Target(next) => {
                                println!("transition to: {}", next);
                                new_states.insert(*next);
                            }
                            Next::Accept => {
                                println!("Accepted!");
                                accepted = true;
                            }
                        }
                    }
                }
            }
        }

        // handle Null states, as they should not consume a character
        let (new_states, accepted_via_null) = self.collapse_null_transitions(new_states);

        (
            new_states,
            accepted || accepted_via_null,
            consumed_a_character,
        )
    }

    /// Searches the input from the beginning, returning a match if one is found.
    pub fn find(&self, input: &[u8]) -> Option<Match> {
        // start state is always the zeroth state
        let mut curr_states = HashSet::from([0]);
        let curr_start = 0;
        let mut accepted = false;
        let accepted_via_null: bool;
        let mut consumed_a_character: bool;

        let mut candidate = MatchCandidate::new();
        candidate.start = curr_start;

        (curr_states, accepted_via_null) = self.collapse_null_transitions(curr_states);

        accepted = accepted || accepted_via_null;

        for (index, &input_char) in input[curr_start..].iter().enumerate() {
            (curr_states, accepted, consumed_a_character) =
                self.do_transition(&curr_states, &input_char, accepted);

            if consumed_a_character && accepted {
                candidate.end = Some(index + 1);
            }

            if curr_states.len() == 0 {
                break;
            }
        }

        if candidate.end.is_some() {
            Some(Match::from_candidate(candidate))
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct MatchCandidate {
    start: usize,
    end: Option<usize>,
}

impl MatchCandidate {
    fn new() -> Self {
        MatchCandidate {
            start: 0,
            end: None,
        }
    }
}

#[derive(Debug)]
pub struct Match {
    pub start: usize,
    pub end: usize,
}

impl Match {
    fn from_candidate(candidate: MatchCandidate) -> Self {
        Match {
            start: candidate.start,
            end: candidate
                .end
                .expect("End required for conversion to a match"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_match(gex_machine: &GexMachine, input: &[u8]) {
        let result = gex_machine.find(input);

        assert!(result.is_some());
        let result_match = result.unwrap();
        assert_eq!(input, &input[result_match.start..result_match.end]);
    }

    #[test]
    fn test_cons() {
        // pattern: `ab+`
        let first = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3)),
                    (Rule::Null, Next::Target(4)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        // pattern: `ab+c*d`
        let second = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3)),
                    (Rule::Null, Next::Target(4)),
                ]),
                State::from_transitions(vec![
                    (Rule::Range(b'c' as u8, b'c' as u8), Next::Target(4)),
                    (Rule::Range(b'd' as u8, b'd' as u8), Next::Target(5)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        // pattern `ab+ab+c*d`
        let result = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3)),
                    (Rule::Null, Next::Target(4)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Target(1 + 4))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2 + 4),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3 + 4),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3 + 4)),
                    (Rule::Null, Next::Target(4 + 4)),
                ]),
                State::from_transitions(vec![
                    (Rule::Range(b'c' as u8, b'c' as u8), Next::Target(4 + 4)),
                    (Rule::Range(b'd' as u8, b'd' as u8), Next::Target(5 + 4)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        assert_eq!(first.cons(second), result);
    }

    #[test]
    fn test_or() {
        // pattern: `ab+`
        let first = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3)),
                    (Rule::Null, Next::Target(4)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        // pattern: `ab+c*d`
        let second = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3)),
                    (Rule::Null, Next::Target(4)),
                ]),
                State::from_transitions(vec![
                    (Rule::Range(b'c' as u8, b'c' as u8), Next::Target(4)),
                    (Rule::Range(b'd' as u8, b'd' as u8), Next::Target(5)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        let result = GexMachine {
            states: vec![
                State::from_transitions(vec![
                    (Rule::Null, Next::Target(1)),
                    (Rule::Null, Next::Target(5)),
                ]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3)),
                    (Rule::Null, Next::Target(4)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Target(10))]), // TODO: add transition to end; how to
                // determine new index
                State::from_transitions(vec![(Rule::Null, Next::Target(1 + 5))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2 + 5),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3 + 5),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3 + 5)),
                    (Rule::Null, Next::Target(4 + 5)),
                ]),
                State::from_transitions(vec![
                    (Rule::Range(b'c' as u8, b'c' as u8), Next::Target(4 + 5)),
                    (Rule::Range(b'd' as u8, b'd' as u8), Next::Target(5 + 5)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        assert_eq!(first.or(second), result);
    }

    #[test]
    fn test_multiple_alternation() {
        let match_a = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        let match_b = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        let match_c = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'c' as u8, b'c' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        let gex_machine = match_a.or(match_b).or(match_c);

        assert_match(&gex_machine, b"a");
        assert_match(&gex_machine, b"b");
        assert_match(&gex_machine, b"c");
    }

    #[test]
    fn test_alternation_with_cons() {
        let match_a = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        let match_b = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        let match_c = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'c' as u8, b'c' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        // pattern: `(a|b)c`
        let gex_machine = match_a.clone().or(match_b.clone()).cons(match_c.clone());

        assert_match(&gex_machine, b"ac");
        assert_match(&gex_machine, b"bc");

        // pattern: `ab|c`
        let gex_machine = match_a.clone().cons(match_b.clone()).or(match_c.clone());

        assert_match(&gex_machine, b"ab");
        assert_match(&gex_machine, b"c");
    }

    #[test]
    fn test_simple_match() {
        // pattern: `ab+`
        let gex_machine = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3)),
                    (Rule::Null, Next::Target(4)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        let test = b"abcd";
        let result = gex_machine.find(test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!(b"ab", &test[test_match.start..test_match.end]);
    }

    #[test]
    fn test_complex_match() {
        // pattern: `ab+c*d`
        let gex_machine = GexMachine {
            states: vec![
                State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
                State::from_transitions(vec![(
                    Rule::Range(b'a' as u8, b'a' as u8),
                    Next::Target(2),
                )]),
                State::from_transitions(vec![(
                    Rule::Range(b'b' as u8, b'b' as u8),
                    Next::Target(3),
                )]),
                State::from_transitions(vec![
                    (Rule::Range(b'b' as u8, b'b' as u8), Next::Target(3)),
                    (Rule::Null, Next::Target(4)),
                ]),
                State::from_transitions(vec![
                    (Rule::Range(b'c' as u8, b'c' as u8), Next::Target(4)),
                    (Rule::Range(b'd' as u8, b'd' as u8), Next::Target(5)),
                ]),
                State::from_transitions(vec![(Rule::Null, Next::Accept)]),
            ],
        };

        let test = b"abcd";
        let result = gex_machine.find(test);

        assert!(result.is_some());
        let test_match = result.unwrap();
        assert_eq!(b"abcd", &test[test_match.start..test_match.end]);

        let test2 = b"abd";
        let result2 = gex_machine.find(test2);

        assert!(result2.is_some());
        let test_match2 = result2.unwrap();
        assert_eq!(b"abd", &test2[test_match2.start..test_match2.end]);
    }
}
