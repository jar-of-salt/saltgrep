use crate::gex::machine::{GexMachine, Next, Rule, State, Transition};
use crate::tokenize::Token;

pub fn machine_for(token: Token, input: &str) -> GexMachine {
    if let Some(range_value) = input[token.input_range()].chars().next() {
        return machine_for_character(range_value);
    }
    // Input range had zero width
    GexMachine::from_states(vec![
        State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
        State::accept_state(),
    ])
}

// TODO: for all of these, try to eliminate the initial null transition
pub fn machine_for_character(character: char) -> GexMachine {
    GexMachine::from_states(vec![
        State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
        State::from_transitions(vec![(
            Rule::Range(character as u32, character as u32, true),
            Next::Target(2),
        )]),
        State::accept_state(),
    ])
}

pub fn wildcard_machine() -> GexMachine {
    GexMachine::from_states(vec![
        State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
        State::from_transitions(vec![(Rule::Range(0, 0x10ffff, true), Next::Target(2))]),
        State::accept_state(),
    ])
}

fn char_class_escape_machine(positive: bool, transitions: Vec<Transition>) -> GexMachine {
    let class_state = if positive {
        State::from_transitions(transitions)
    } else {
        State::short_circuit_from_transitions(transitions)
    };
    let states = vec![
        State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
        class_state,
        State::accept_state(),
    ];
    GexMachine::from_states(states)
}

pub fn word_char_machine(positive: bool) -> GexMachine {
    let mut transitions = vec![(Rule::IsWord(positive), Next::Target(2))];
    if positive {
        transitions.push((Rule::Range('_' as u32, '_' as u32, true), Next::Target(2)));
    } else {
        transitions.push((Rule::Not('_' as u32), Next::Target(2)));
    }
    char_class_escape_machine(positive, transitions)
}

pub fn digit_char_machine(positive: bool) -> GexMachine {
    let transitions = vec![(Rule::IsDigit(positive), Next::Target(2))];
    char_class_escape_machine(positive, transitions)
}

pub fn whitespace_char_machine(positive: bool) -> GexMachine {
    let transitions = vec![(Rule::IsWhitespace(positive), Next::Target(2))];
    char_class_escape_machine(positive, transitions)
}

pub fn manual_character_class_machine(positive: bool, input_class: &str) -> GexMachine {
    let start = if positive { 1 } else { 2 };
    let class_contents = &input_class[start..input_class.len() - 1];

    let mut result_stack: Vec<char> = Vec::with_capacity(50);
    let mut ranges: Vec<Rule> = Vec::with_capacity(50);

    let mut peekable_chars = class_contents.chars().peekable();

    while let Some(character) = peekable_chars.next() {
        if '\\' == character {
            result_stack.push(
                peekable_chars
                    .next()
                    .expect("No corresponding escaped character found"),
            );
        } else if '-' == character {
            let next_char = peekable_chars.next();
            let prev_char = result_stack.last();

            match (next_char, prev_char) {
                // The range has endpoints
                (Some(next), Some(prev)) => {
                    // advance to the end of the range
                    if next < *prev {
                        panic!("Range is out of order in character set: {}", input_class);
                    }
                    ranges.push(Rule::Range(*prev as u32, next as u32, positive));
                }
                //The character set starts with a hyphen
                (Some(next), None) => {
                    result_stack.push('-');
                    result_stack.push(next);
                }
                //The character set ends with a hyphen
                (None, Some(_)) => {
                    result_stack.push('-');
                }
                _ => (),
            }
        } else {
            result_stack.push(character);
        }
        // if escaped, get next
        // else push to result stack
    }

    let transitions: Vec<Transition> = ranges
        .into_iter()
        .chain(
            result_stack
                .into_iter()
                .map(|character| Rule::Range(character as u32, character as u32, positive)),
        )
        .map(|range| (range, Next::Target(2)))
        .collect();

    GexMachine::from_states(vec![
        State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
        if positive {
            State::from_transitions(transitions)
        } else {
            State::short_circuit_from_transitions(transitions)
        },
        State::accept_state(),
    ])
}
