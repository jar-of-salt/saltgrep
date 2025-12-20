use crate::gex::machine::{GexMachine, Next, Rule, State};
use crate::matcher::{Match, Matcher};
use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq, Eq, Clone)]
struct MatchCandidate {
    start: usize,
    end: Option<usize>,
}

impl MatchCandidate {
    fn new() -> Self {
        MatchCandidate::with_start(0)
    }

    fn with_start(start: usize) -> Self {
        MatchCandidate { start, end: None }
    }
}

impl Match {
    fn from_candidate(candidate: &MatchCandidate) -> Self {
        Match {
            start: candidate.start,
            end: candidate
                .end
                .expect("End required for conversion to a match"),
        }
    }
}

enum Special {
    GroupStart(u16),
    GroupEnd(u16),
    StartAnchor,
    EndAnchor,
}

struct GexMatcher {
    captures: Option<HashMap<u16, MatchCandidate>>,
}

impl GexMatcher {
    fn unwrap_captures(&self) -> HashMap<u16, Match> {
        let mut actual_captures: HashMap<u16, Match> = HashMap::new();
        for (&idx, candidate) in self.captures.as_ref().unwrap().iter() {
            if let Some(_) = candidate.end {
                actual_captures.insert(idx, Match::from_candidate(&candidate));
            }
        }
        actual_captures
    }
}

/// Matcher-trait-specific impl for GexMachine
impl GexMachine {
    /// Evaluate whether a given input matches the given rule.
    ///
    /// Null transition rules will always evaluate as falsy since they need to be collapsed to next
    /// states without consuming a character, and this is handled separately.
    fn evaluate_rule(rule: &Rule, given: &char) -> bool {
        match rule {
            Rule::Range(start, end, positive) => {
                (*start <= *given as u32 && *given as u32 <= *end) ^ !positive
            }
            Rule::Not(value) => *given as u32 != *value,
            Rule::IsWord(positive) => given.is_alphanumeric() ^ !positive,
            Rule::IsDigit(positive) => given.is_numeric() ^ !positive,
            Rule::IsWhitespace(positive) => given.is_whitespace() ^ !positive,
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
                let mut curr_state_collapsed = false;
                for (rule, transition) in state.transitions.iter() {
                    if let Rule::Null = rule {
                        curr_state_collapsed = true;
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
                if curr_state_collapsed {
                    collapsed_states.remove(&last_state);
                }
            }
        }
        (collapsed_states, accept)
    }

    fn evaluate_state_transitions(
        state: &State,
        input_char: &char,
        new_states: &mut HashSet<usize>,
        consumed_a_character: &mut bool,
        accepted: &mut bool,
    ) -> u8 {
        let mut states_to_add: HashSet<usize> = HashSet::new();

        for (rule, transition) in state.transitions.iter() {
            if GexMachine::evaluate_rule(rule, input_char) {
                *consumed_a_character = true;
                match transition {
                    Next::Target(next) => {
                        states_to_add.insert(*next);
                    }
                    Next::Accept => {
                        *accepted = true;
                    }
                }
            } else if state.short_circuit() {
                *consumed_a_character = false;
                return 0;
            }
        }
        new_states.extend(states_to_add);
        1
    }

    fn capture_group(matcher: &mut GexMatcher, state: &State, position: usize) {
        let group_number = state.group_number();
        if group_number != 0 {
            let captures = matcher.captures.as_mut().unwrap();
            match state.close_group() {
                0 => {
                    captures.insert(group_number, MatchCandidate::with_start(position));
                }
                1 => {
                    captures.get_mut(&group_number).as_mut().unwrap().end = Some(position);
                }
                _ => panic!("unrecognized group flag"),
            }
        }
    }

    fn evaluate_state_flags(matcher: &mut GexMatcher, state: &State, position: usize) -> bool {
        let mut short_circuit = false;
        if matcher.captures.is_some() {
            GexMachine::capture_group(matcher, state, position);
        }
        short_circuit
    }

    /// Attempts to consume an input and determines the set of states after the transition.
    fn transition(
        &self,
        curr_states: &HashSet<usize>,
        input_char: &char,
        matcher: &mut GexMatcher,
        mut accepted: bool,
    ) -> (HashSet<usize>, bool, bool) {
        let mut new_states: HashSet<usize> = HashSet::new();

        let mut consumed_a_character = false;

        for state_label in curr_states.iter() {
            if let Some(state) = self.states.get(*state_label) {
                let mut short_circuit = false;
                let mut states_to_add: HashSet<usize> = HashSet::new();

                for (rule, transition) in state.transitions.iter() {
                    if GexMachine::evaluate_rule(rule, input_char) {
                        consumed_a_character = true;
                        match transition {
                            Next::Target(next) => {
                                states_to_add.insert(*next);
                            }
                            Next::Accept => {
                                accepted = true;
                            }
                        }
                    } else if state.short_circuit() {
                        consumed_a_character = false;
                        short_circuit = true;
                    }
                }
                if short_circuit {
                    break;
                }
                new_states.extend(states_to_add);
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
}

impl Matcher for GexMachine {
    /// Searches the input from the beginning, returning a match if one is found.
    fn find(&self, input: &str) -> Option<Match> {
        // start state is always the zeroth state
        let mut curr_states = HashSet::from([0]);
        let start_position = 0;
        let mut position = start_position;
        let mut accepted = false;
        let accepted_via_null: bool;
        let mut consumed_a_character: bool;
        let mut matcher = GexMatcher {
            captures: Some(HashMap::new()),
        };

        let mut candidate = MatchCandidate::new();
        candidate.start = start_position;

        (curr_states, accepted_via_null) = self.collapse_null_transitions(curr_states);

        if accepted_via_null {
            candidate.end = Some(start_position);
        }

        accepted = accepted || accepted_via_null;

        for input_char in input[start_position..].chars() {
            let char_len = input_char.len_utf8();
            (curr_states, accepted, consumed_a_character) =
                self.transition(&curr_states, &input_char, &mut matcher, accepted);

            if consumed_a_character {
                position += char_len;
                if accepted {
                    candidate.end = Some(position);
                }
            }

            if curr_states.len() == 0 {
                break;
            }
        }

        if candidate.end.is_some() {
            Some(Match::from_candidate(&candidate))
        } else {
            None
        }
    }
}
