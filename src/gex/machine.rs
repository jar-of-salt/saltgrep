use crate::gex::features::GexFeatures;
use std::collections::HashMap;
use std::convert::TryInto;

// NOTE: this actually forces us to use UTF-8
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Rule {
    Range(u32, u32, bool),
    Not(u32),
    IsWord(bool),
    IsDigit(bool),
    IsWhitespace(bool),
    Null,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Next {
    Target(usize),
    Accept,
}

/// State Flag spec: 64 bit unsigned integer; final 16 bits are reserved for capturing group
pub enum FlagShifts {
    ShortCircuit = 0,
    CloseGroup = 1,
    CapturingGroup = 48,
}

pub enum FlagMasks {
    ShortCircuit = 0x1,
    CloseGroup = 0x2,
    EndAnchor = 0x5,
    CapturingGroup = (0xFFFF << FlagShifts::CapturingGroup as u64),
}

pub type Transition = (Rule, Next);

/// Extensible state struct.
/// Goal is to add ability to mark states as the beginning of a group etc, right now just a dumb
/// state machine state.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct State {
    pub transitions: Vec<Transition>,
    /// `true` indicates that a single falsy rule in the transitions should cause all other
    /// potential rules in this state to evaluate as falsy.
    pub flags: u64,
}

enum Group {
    Start(u16),
    End(u16),
}

impl State {
    pub fn from_transitions(transitions: Vec<Transition>) -> Self {
        State {
            transitions,
            flags: 0x0,
        }
    }

    pub fn short_circuit_from_transitions(transitions: Vec<Transition>) -> Self {
        State {
            transitions,
            flags: FlagMasks::ShortCircuit as u64,
        }
    }

    pub fn accept_state() -> Self {
        State {
            transitions: vec![(Rule::Null, Next::Accept)],
            flags: 0x0,
        }
    }

    pub fn push(&mut self, transition: Transition) {
        self.transitions.push(transition)
    }

    pub fn short_circuit(&self) -> bool {
        (self.flags & FlagMasks::ShortCircuit as u64) != 0
    }

    pub fn group_number(&self) -> u16 {
        (self.flags >> FlagShifts::CapturingGroup as usize) as u16
    }

    pub fn close_group(&self) -> u8 {
        ((self.flags & FlagMasks::CloseGroup as u64) >> FlagShifts::CloseGroup as u64) as u8
    }
}

fn states_shifter(shift: usize) -> impl Fn(State) -> State {
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

/// A Non-deterministic Finite Automata for acceptance evaluation is represented here.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GexMachine {
    /// Each state is a vector of unicode ranges and the state they map to
    pub states: Vec<State>,
    pub(super) features: GexFeatures,
    max_group_index: u16,
}

// TODO: implement find w/ explain -> might be hard with this implementation

/// NFA implementation for solving regex.
/// Supports operations to build composite machines via concatenation and alternation.
impl GexMachine {
    pub fn from_states(states: Vec<State>) -> Self {
        GexMachine {
            states,
            features: GexFeatures::new(),
            max_group_index: 0,
        }
    }
    /// Create NFA with the given states vec capacity.
    /// The consuming regex matcher should make a best-guess at the eventual size of the NFA to
    /// avoid excessive reallocation.
    pub fn with_capacity(cap: usize) -> Self {
        let mut states = Vec::with_capacity(cap);
        states.push(State::from_transitions(vec![(Rule::Null, Next::Target(1))]));
        states.push(State::accept_state());
        GexMachine::from_states(states)
    }

    pub fn default() -> Self {
        // TODO: when making a machine from a pattern, have a reasonable guess at the size of the
        // NFA
        GexMachine::with_capacity(1_000_000)
    }

    pub fn size(&self) -> usize {
        self.states.len()
    }

    fn add_shifted_flags(
        &mut self,
        other_state_flags: HashMap<usize, Vec<u64>>,
        old_accept_idx: usize,
        maintain_root: bool,
    ) {
        let group_shift = self.max_group_index;
        for (state_idx, flags_vec) in other_state_flags.into_iter() {
            let new_idx = if maintain_root && state_idx == 0 {
                state_idx
            } else {
                state_idx + old_accept_idx
            };
            let mut shifted_flags = Some(
                flags_vec
                    .into_iter()
                    .filter(|flags| GexFeatures::group_number(*flags) != 0)
                    .map(|mut flags| {
                        let big_new_number: u64 =
                            (GexFeatures::group_number(flags) as u64) + group_shift as u64;
                        let cast_result: Result<u16, _> = big_new_number.try_into();
                        match cast_result {
                            Ok(new_group_number) => {
                                flags &= flags & !(FlagMasks::CapturingGroup as u64);
                                flags |=
                                    (new_group_number as u64) << FlagShifts::CapturingGroup as u64;
                                flags
                            }
                            Err(e) => {
                                panic!("New group number exceeds size of u16; error: {:?}", e)
                            }
                        }
                    })
                    .collect::<Vec<u64>>(),
            );

            self.features
                .state_flags
                .entry(new_idx)
                .and_modify(|entry| entry.extend(shifted_flags.take().unwrap()))
                .or_insert_with(|| shifted_flags.take().unwrap());
        }
    }

    /// Concatenate the current NFA with another.
    /// The other NFA will be appended to the receiver.
    /// TODO: improve this so the current accept state doesn't become a null transition
    pub fn cons(mut self, other: GexMachine) -> GexMachine {
        let old_accept_idx = self.size() - 1;
        // IMPORTANT Assumption: the last state always contains a singular Accept
        self.states.pop();

        let new_states = other.states.into_iter().map(states_shifter(old_accept_idx));

        self.states.extend(new_states);

        self.add_shifted_flags(other.features.state_flags, old_accept_idx, false);

        self.max_group_index += other.max_group_index;

        self
    }

    // TODO: consider allowing arbitrary final states, not just a singular accept?

    /// Alternate the current NFA with another.
    /// The other NFA will be added as the "right hand side" entry of the alternation,
    /// and the receiver will be the "left hand side."
    /// TODO: determine if taking ownership of other is a good idea...
    pub fn or(mut self, mut other: GexMachine) -> GexMachine {
        self.states.reserve(other.size());

        let other_start = self.size();

        self.states[0].push((Rule::Null, Next::Target(other_start)));

        let new_accept_idx = self.size() + other.states.len() - 1;
        let old_accept_idx = self.size() - 1;

        let old_accept = self
            .states
            .last_mut()
            .expect("A non-empty set of states is required")
            .transitions
            .last_mut()
            .expect("An Accept state is required");

        old_accept.1 = Next::Target(new_accept_idx);

        // This can replace the above if we allow the final state to contain more than a single
        // transition
        // let final_state = self
        //     .states
        //     .last_mut()
        //     .expect("A non-empty set of states is required");

        // for transition in final_state.transitions.iter_mut() {
        //     if let (_, Next::Accept) = transition {
        //         transition.1 = Next::Target(new_accept_idx);
        //     }
        // }

        let new_states = other.states.into_iter().map(states_shifter(other_start));

        self.states.extend(new_states);

        self.add_shifted_flags(other.features.state_flags, old_accept_idx, true);

        self.max_group_index += other.max_group_index;

        self
    }

    // TODO: implement non-capturing groups
    pub fn group(mut self) -> Self {
        self.max_group_index += 1;
        let new_group_number = (self.max_group_index as u64) << FlagShifts::CapturingGroup as u64;
        let last_idx = self.states.len() - 1;

        let start_flag = new_group_number;
        let end_flag = new_group_number | FlagMasks::CloseGroup as u64;

        self.features
            .state_flags
            .entry(0)
            .and_modify(|flags| {
                flags.push(start_flag);
            })
            .or_insert(vec![start_flag]);

        self.features
            .state_flags
            .entry(last_idx)
            .and_modify(|flags| {
                flags.push(end_flag);
            })
            .or_insert(vec![end_flag]);

        self
    }

    fn accept_zero(mut self) -> Self {
        let new_accept_idx = self.size();
        self.states[0].push((Rule::Null, Next::Target(new_accept_idx)));
        self
    }

    fn accept_repeats(mut self) -> Self {
        let new_accept_idx = self.size();
        self.states[new_accept_idx - 1].push((Rule::Null, Next::Target(0)));
        self
    }

    fn finalize_quantifier(mut self) -> Self {
        let new_accept_idx = self.size();
        for transition in self.states[new_accept_idx - 1].transitions.iter_mut() {
            if let (_, Next::Accept) = transition {
                transition.1 = Next::Target(new_accept_idx);
            }
        }
        self.states.push(State::accept_state());
        self
    }

    pub fn zero_or_more(self) -> Self {
        self.accept_zero().accept_repeats().finalize_quantifier()
    }

    pub fn one_or_more(self) -> Self {
        self.accept_repeats().finalize_quantifier()
    }

    pub fn zero_or_one(self) -> Self {
        self.accept_zero().finalize_quantifier()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::Matcher;

    fn assert_match(gex_machine: &GexMachine, input: &str, match_string: &str) {
        let result = gex_machine.find(input);

        assert!(result.is_some());
        let result_match = result.unwrap();
        assert_eq!(match_string, &input[result_match.start..result_match.end]);
    }

    fn assert_full_match(gex_machine: &GexMachine, input: &str) {
        assert_match(gex_machine, input, input);
    }

    fn assert_no_match(gex_machine: &GexMachine, input: &str) {
        let result = gex_machine.find(input);

        assert!(result.is_none());
    }

    fn machine_for(atom: char) -> GexMachine {
        GexMachine::from_states(vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(
                Rule::Range(atom as u32, atom as u32, true),
                Next::Target(2),
            )]),
            State::accept_state(),
        ])
    }

    #[test]
    fn test_cons() {
        let gex_machine = machine_for('a')
            .cons(machine_for('b'))
            .cons(machine_for('c'));

        assert_full_match(&gex_machine, "abc");
        assert_no_match(&gex_machine, "cba");
    }

    #[test]
    fn test_or() {
        let gex_machine = machine_for('a').or(machine_for('b'));

        assert_full_match(&gex_machine, "a");
        assert_full_match(&gex_machine, "b");
        assert_match(&gex_machine, "aab", "a");
        assert_match(&gex_machine, "bab", "b");
        assert_match(&gex_machine, "babdef", "b");
        assert_no_match(&gex_machine, "c");
        assert_no_match(&gex_machine, "cdef");
    }

    #[test]
    fn test_zero_or_more() {
        let gex_machine = machine_for('a').zero_or_more();
        assert_full_match(&gex_machine, "a");
        assert_full_match(&gex_machine, "");
        assert_full_match(&gex_machine, "aa");
        assert_full_match(&gex_machine, "aaaaa");
        assert_match(&gex_machine, "aab", "aa");
        assert_match(&gex_machine, "baaaaa", "");
        assert_match(&gex_machine, "c", "");
    }

    #[test]
    fn test_zero_or_one() {
        let gex_machine = machine_for('a').zero_or_one();

        assert_full_match(&gex_machine, "a");
        assert_full_match(&gex_machine, "");
        assert_match(&gex_machine, "aa", "a");
        assert_match(&gex_machine, "aaaaa", "a");
        assert_match(&gex_machine, "aab", "a");
        assert_match(&gex_machine, "baaaaa", "");
        assert_match(&gex_machine, "c", "");
    }

    #[test]
    fn test_one_or_more() {
        let gex_machine = machine_for('a').one_or_more();

        assert_full_match(&gex_machine, "a");
        assert_full_match(&gex_machine, "aa");
        assert_full_match(&gex_machine, "aaaaa");
        assert_match(&gex_machine, "aab", "aa");
        assert_no_match(&gex_machine, "baaaaa");
        assert_no_match(&gex_machine, "");
    }

    #[test]
    fn test_multiple_alternation() {
        let gex_machine = machine_for('a').or(machine_for('b')).or(machine_for('c'));

        assert_full_match(&gex_machine, "a");
        assert_full_match(&gex_machine, "b");
        assert_full_match(&gex_machine, "c");
        assert_no_match(&gex_machine, "d");
    }

    #[test]
    fn test_complex_composition() {
        // pattern: `(a|b)+ca?b*`
        let gex_machine = machine_for('a')
            .or(machine_for('b'))
            .one_or_more()
            .cons(machine_for('c'))
            .cons(machine_for('a').zero_or_one())
            .cons(machine_for('b').zero_or_more());

        assert_full_match(&gex_machine, "ac");
        assert_full_match(&gex_machine, "bc");
        assert_full_match(&gex_machine, "abbacabb");
        assert_full_match(&gex_machine, "bcbbbb");
        assert_full_match(&gex_machine, "baaaabcabbbb");
    }

    #[test]
    fn test_state_short_circuit() {
        let state = State::short_circuit_from_transitions(vec![]);

        assert!(state.short_circuit());
    }
}
