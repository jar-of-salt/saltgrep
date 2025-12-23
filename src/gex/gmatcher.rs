use crate::gex::machine::{GexMachine, Next, Rule};
use crate::matcher::{Match, Matcher};
use std::collections::{HashMap, HashSet};
use std::iter::once;

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
    fn collapse_null_transitions(
        &self,
        curr_states: HashSet<usize>,
        position: usize,
        matcher: &mut GexMatcher,
    ) -> (HashSet<usize>, bool) {
        // Keep track of visited states to prevent uncontrolled recursive collapse.
        let mut visited = HashSet::<usize>::new();
        let mut collapsed_states = HashSet::<usize>::new();
        let mut states = curr_states.into_iter().collect::<Vec<usize>>();

        let mut accept = false;

        while let Some(last_state_label) = states.pop() {
            collapsed_states.insert(last_state_label);
            println!("A state: {}, position: {}", last_state_label, position);

            if visited.contains(&last_state_label) {
                continue;
            }
            visited.insert(last_state_label);

            if let Some(state) = self.states.get(last_state_label) {
                println!("B state: {}, position: {}", last_state_label, position);
                self.evaluate_state_flags(matcher, last_state_label, position);
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
                        collapsed_states.remove(&last_state_label);
                    }
                }
            }
        }
        (collapsed_states, accept)
    }

    fn capture_group(&self, matcher: &mut GexMatcher, state_label: usize, position: usize) {
        if let Some(group_numbers) = self.features.group_numbers(state_label) {
            for (group_number, close_group_flag) in group_numbers {
                let captures = matcher.captures.as_mut().unwrap();
                match close_group_flag {
                    0 => {
                        println!("open {} at {}", group_number, position);
                        captures.insert(group_number, MatchCandidate::with_start(position));
                    }
                    1 => {
                        println!("close {} at {}", group_number, position);
                        if group_number == 3 {
                            // panic!("debug");
                        }
                        captures.get_mut(&group_number).as_mut().unwrap().end = Some(position);
                    }
                    _ => panic!("unrecognized group flag"),
                }
            }
        }
    }

    fn evaluate_state_flags(&self, matcher: &mut GexMatcher, state_label: usize, position: usize) {
        if matcher.captures.is_some() {
            self.capture_group(matcher, state_label, position);
        }
    }

    /// Attempts to consume an input and determines the set of states after the transition.
    fn do_transition(
        &self,
        curr_states: &HashSet<usize>,
        input_char: &char,
        matcher: &mut GexMatcher,
        position: usize,
        mut accepted: bool,
    ) -> (HashSet<usize>, bool, usize) {
        let mut new_states: HashSet<usize> = HashSet::new();

        let mut consumed_a_character = false;
        let mut new_position = position;

        for state_label in curr_states.iter() {
            if let Some(state) = self.states.get(*state_label) {
                let mut short_circuit = false;
                let mut states_to_add: HashSet<usize> = HashSet::new();

                self.evaluate_state_flags(matcher, *state_label, position);
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
                        break;
                    }
                }

                if short_circuit {
                    break;
                }
                new_states.extend(states_to_add);
            }
        }

        if consumed_a_character {
            new_position += 1;
        }

        // handle Null states, as they should not consume a character
        let (new_states, accepted_via_null) =
            self.collapse_null_transitions(new_states, new_position, matcher);

        (new_states, accepted || accepted_via_null, new_position)
    }

    fn run_machine(&self, input: &str, matcher: &mut GexMatcher) -> Option<Match> {
        // start state is always the zeroth state
        let mut curr_states = HashSet::from([0]);
        let start_position = 0;
        let mut position = start_position;
        let mut next_position = position;
        let mut accepted = false;
        let accepted_via_null: bool;
        let mut consumed_a_character: bool;

        let mut candidate = MatchCandidate::new();
        candidate.start = start_position;

        (curr_states, accepted_via_null) =
            self.collapse_null_transitions(curr_states, position, matcher);

        if accepted_via_null {
            candidate.end = Some(start_position);
        }

        // accepted = accepted || accepted_via_null;

        for input_char in input[start_position..].chars() {
            let char_len = input_char.len_utf8();
            (curr_states, accepted, next_position) =
                self.do_transition(&curr_states, &input_char, matcher, position, accepted);

            if next_position > position {
                position = next_position;
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

impl Matcher for GexMachine {
    fn find_at(&self, input: &str, at: usize) -> Option<Match> {
        let mut matcher = GexMatcher { captures: None };
        let start_input = &input[at..];

        // Allow for the case of an empty string match
        // TODO: test this at a different location than the start of the string w/ a zero or more
        // quantifier
        once((0, '\0'))
            .chain(start_input.char_indices())
            .map(|(idx, _)| {
                self.run_machine(&start_input[idx..], &mut matcher)
                    .map(|found| (idx, found))
            })
            .find(Option::is_some)
            .flatten()
            .map(|(offset, found)| found.shift(offset + at))
    }

    /// Searches the input from the beginning, returning a match if one is found.
    fn find(&self, input: &str) -> Option<Match> {
        self.find_at(input, 0)
    }

    fn captures(&self, input: &str) -> Option<HashMap<u16, Match>> {
        let mut matcher = GexMatcher {
            captures: Some(HashMap::new()),
        };

        self.run_machine(input, &mut matcher).map(|root_match| {
            let mut captures = matcher.unwrap_captures();
            captures.insert(0, root_match);
            captures
        })
    }
}
