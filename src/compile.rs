use crate::gex::{GexMachine, Next, Rule, State, Transition};
use crate::railroad::{Ast, AstNode};
use crate::tokenize::{tokenize, CharacterClassType, LiteralType, QuantifierType, Token};

fn machine_for(token: Token, input: &str) -> GexMachine {
    let range_value = input[token.input_range()]
        .chars()
        .next()
        .expect("Invalid input");
    return machine_for_character(range_value);
}

fn machine_for_character(character: char) -> GexMachine {
    GexMachine::from_states(vec![
        State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
        State::from_transitions(vec![(
            Rule::Range(character as u32, character as u32, true),
            Next::Target(2),
        )]),
        State::accept_state(),
    ])
}

fn wildcard_machine() -> GexMachine {
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

fn word_char_machine(positive: bool) -> GexMachine {
    let mut transitions = vec![(Rule::IsWord(positive), Next::Target(2))];
    if positive {
        transitions.push((Rule::Range('_' as u32, '_' as u32, true), Next::Target(2)));
    } else {
        transitions.push((Rule::Not('_' as u32), Next::Target(2)));
    }
    char_class_escape_machine(positive, transitions)
}

fn digit_char_machine(positive: bool) -> GexMachine {
    let transitions = vec![(Rule::IsDigit(positive), Next::Target(2))];
    char_class_escape_machine(positive, transitions)
}

fn whitespace_char_machine(positive: bool) -> GexMachine {
    let transitions = vec![(Rule::IsWhitespace(positive), Next::Target(2))];
    char_class_escape_machine(positive, transitions)
}

fn manual_character_class_machine(positive: bool, input_class: &str) -> GexMachine {
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

// NOTE: maybe it would have been easier to figure out token/astnode type layout by writing this
// first??
pub fn compile(input: &str) -> GexMachine {
    let tokens = tokenize(input);
    let ast = Ast::from_tokens(tokens);

    let mut combination_stack: Vec<GexMachine> = Vec::with_capacity(2);

    for ast_node in ast.0.iter() {
        match ast_node {
            AstNode::Literal(ltype, token) => match ltype {
                LiteralType::Wildcard => combination_stack.push(wildcard_machine()),
                LiteralType::Character => combination_stack.push(machine_for(*token, input)),
                LiteralType::EscapedCharacter => {
                    let escaped = input[token.input_range()]
                        .chars()
                        .nth(1)
                        .expect("Unfinished escape sequence during compilation");
                    combination_stack.push(machine_for_character(escaped));
                }
                LiteralType::CharacterClass(class_type, positive) => {
                    let class_machine = match class_type {
                        CharacterClassType::Word => word_char_machine(*positive),
                        CharacterClassType::Digit => digit_char_machine(*positive),
                        CharacterClassType::Whitespace => whitespace_char_machine(*positive),
                        CharacterClassType::Manual => {
                            manual_character_class_machine(*positive, &input[token.input_range()])
                        }
                    };
                    combination_stack.push(class_machine);
                }
                LiteralType::EmptyString => panic!("Empty string not implemented"),
            },
            AstNode::Quantifier(qtype, _) => match qtype {
                QuantifierType::ZeroOrMore => {
                    let zero_or_more = combination_stack
                        .pop()
                        .expect("Operand expected")
                        .zero_or_more();
                    combination_stack.push(zero_or_more);
                }
                QuantifierType::OneOrMore => {
                    let one_or_more = combination_stack
                        .pop()
                        .expect("Operand expected")
                        .one_or_more();
                    combination_stack.push(one_or_more);
                }
                QuantifierType::ZeroOrOne => {
                    let zero_or_one = combination_stack
                        .pop()
                        .expect("Operand expected")
                        .zero_or_one();
                    combination_stack.push(zero_or_one);
                }
            },
            AstNode::Cons(_, _) => {
                let right = combination_stack.pop().expect("No RHS for cons operation.");
                let left = combination_stack.pop().expect("No LHS for cons operation.");
                combination_stack.push(left.cons(right));
            }
            AstNode::Alternation(_, _) => {
                let right = combination_stack.pop().expect("No RHS for cons operation.");
                let left = combination_stack.pop().expect("No LHS for cons operation.");
                combination_stack.push(left.or(right));
            }
            AstNode::Group(_) => {
                let group = combination_stack.pop().expect("Operand expected").group();
                combination_stack.push(group);
            }
        }
    }
    combination_stack.pop().expect("No NFA created")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::{Match, Matcher};

    fn assert_match(pattern: &str, input: &str, match_string: &str) {
        let gex_machine = compile(pattern);
        let result = gex_machine.find(input);

        assert!(result.is_some());
        let result_match = result.unwrap();
        assert_eq!(match_string, &input[result_match.start..result_match.end]);
    }

    fn assert_full_match(pattern: &str, input: &str) {
        assert_match(pattern, input, input);
    }

    fn assert_no_match(pattern: &str, input: &str) {
        let gex_machine = compile(pattern);
        let result = gex_machine.find(input);

        assert!(result.is_none());
    }

    #[test]
    fn test_nfa() {
        assert_full_match(r"abcd+(efg)|i", r"i");
    }

    #[test]
    fn test_exotic_cases() {
        assert_full_match(r"ab\c.d+(efg)|i", r"abcxdddefg");
    }

    #[test]
    fn test_word_char_class() {
        assert_full_match(r"\w", r"a");
        assert_full_match(r"\w+", r"abfhkg10235_1204");

        assert_no_match(r"\w", r"-");
        assert_no_match(r"\w+", r"%^$//_0-");

        assert_full_match(r"\W", r"&");
        assert_match(r"\W+", r"%^$//_0-", r"%^$//");

        assert_no_match(r"\W", r"a");
        assert_no_match(r"\W+", r"abckjdjfk");
    }

    #[test]
    fn test_digit_char_class() {
        assert_full_match(r"\d", r"0");
        assert_full_match(r"\d+", r"1234567890");

        assert_no_match(r"\d", r"^");
        assert_no_match(r"\d+", r"abc(*");

        assert_full_match(r"\D", r"a");
        assert_full_match(r"\D+", r"cddfi*&^w");

        assert_no_match(r"\D", r"1");
        assert_no_match(r"\D+", r"12345");
    }

    #[test]
    fn test_whitespace_char_class() {
        assert_full_match(r"\s", r" ");
        assert_full_match(r"\s+", " \n");

        assert_no_match(r"\s", r"d");
        assert_no_match(r"\s+", r"abc(*");

        assert_full_match(r"\S", r"d");
        assert_full_match(r"\S+", r"cddfi*&^w");

        assert_no_match(r"\S", "\n");
        assert_no_match(r"\S+", "  \n  ");
    }

    #[test]
    fn test_basic_character_class() {
        assert_full_match(r"[abc]", r"b");
        assert_full_match(r"[a-z]", r"x");
        assert_full_match(r"[a-zA-Z]", r"Y");

        assert_no_match(r"[abc]", r"d");
        assert_no_match(r"[a-z]", r"X");
        assert_no_match(r"[a-zA-Z]", r"5");
    }

    #[test]
    fn test_basic_negative_character_class() {
        assert_full_match(r"[^abc]", r"d");
        assert_full_match(r"[^a-z]", r"A");
        assert_full_match(r"[^a-zA-Z]", r"5");

        assert_no_match(r"[^abc]", r"c");
        assert_no_match(r"[^a-z]", r"a");
        assert_no_match(r"[^a-zA-Z]", r"X");
    }

    #[test]
    fn test_quantified_character_class() {
        assert!(compile(r"[abc]+").find(r"abcabccba").is_some());
        assert!(compile(r"[^abc]+").find(r"def").is_some());
        assert_full_match(r"[a-z]*", r"a");
        assert_full_match(r"[^a-z]?", r"A");
        assert!(compile(r"[a-zA-Z]+").find(r"abcdAXZ").is_some());
        assert!(compile(r"[^a-zA-Z]*").find(r"52787&^%$").is_some());

        assert!(compile(r"[abc]+").find(r"defdfk").is_none());
        assert!(compile(r"[^abc]+").find(r"abc").is_none());
        assert!(compile(r"[a-z]+").find(r"ABC").is_none());
        assert!(compile(r"[^a-z]+").find(r"abc").is_none());
        assert!(compile(r"[a-zA-Z]+").find(r"1203845").is_none());
        assert!(compile(r"[^a-zA-Z]+").find(r"abcACCD").is_none());
    }

    #[test]
    fn test_simple_capturing_group() {
        let machine = compile(r"(abc)");
        let wrapped_captures = machine.captures(r"abcdfdefg");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert!(*captures.get(&1).unwrap() == Match { start: 0, end: 3 });
    }

    #[test]
    fn test_simple_staggered_capturing_group() {
        let machine = compile(r"123(abc)");
        let wrapped_captures = machine.captures(r"123abcdfdefg");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert!(*captures.get(&1).unwrap() == Match { start: 3, end: 6 });
    }

    #[test]
    fn test_capturing_group() {
        assert_full_match(r"(abc)df(defg)(123)", r"abcdfdefg123");

        let machine = compile(r"(abc)df(defg)(123)");

        let wrapped_captures = machine.captures(r"abcdfdefg123");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert!(*captures.get(&1).unwrap() == Match { start: 0, end: 3 });
        assert!(*captures.get(&2).unwrap() == Match { start: 5, end: 9 });
        assert!(*captures.get(&3).unwrap() == Match { start: 9, end: 12 });
    }

    #[test]
    fn test_capturing_group_with_alternation() {
        let machine = compile(r"(abc)df(defg)|(123)");

        let wrapped_captures = machine.captures(r"abcdfdefg123");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert!(*captures.get(&1).unwrap() == Match { start: 0, end: 3 });
        assert!(*captures.get(&2).unwrap() == Match { start: 5, end: 9 });
    }
}
