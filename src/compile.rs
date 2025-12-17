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
    return GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(
                Rule::Range(character as u32, character as u32, true),
                Next::Target(2),
            )]),
            State::accept_state(),
        ],
    };
}

fn wildcard_machine() -> GexMachine {
    GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(Rule::Range(0, 0x10ffff, true), Next::Target(2))]),
            State::accept_state(),
        ],
    }
}

fn word_char_machine(positive: bool) -> GexMachine {
    let mut machine = GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![
                (Rule::IsAlphabetic(positive), Next::Target(2)),
                (Rule::IsDigit(positive), Next::Target(2)),
            ]),
            State::accept_state(),
        ],
    };

    if positive {
        machine.states[1].push((Rule::Range('_' as u32, '_' as u32, true), Next::Target(2)));
    }

    machine
}

fn digit_char_machine(positive: bool) -> GexMachine {
    GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(Rule::IsDigit(positive), Next::Target(2))]),
            State::accept_state(),
        ],
    }
}

fn whitespace_char_machine(positive: bool) -> GexMachine {
    GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(Rule::IsWhitespace(positive), Next::Target(2))]),
            State::accept_state(),
        ],
    }
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

    let machine = GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            if positive {
                State::from_transitions(transitions)
            } else {
                State::short_circuit_from_transitions(transitions)
            },
            State::accept_state(),
        ],
    };

    machine
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
                    println!("positive: {}", *positive);
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

    #[test]
    fn test_nfa() {
        assert!(compile(r"abcd+(efg)|i").find(r"i").is_some());
    }

    #[test]
    fn test_exotic_cases() {
        println!("machine: {:?}", compile(r"ab\c.d+(efg)|i"));
        assert!(compile(r"ab\c.d+(efg)|i").find(r"abcxdddefg").is_some());
    }

    #[test]
    fn test_word_char_class() {
        assert!(compile(r"\w").find(r"-").is_none());
        assert!(compile(r"\w").find(r"a").is_some());
        assert!(compile(r"\w+").find(r"abfhkg10235_1204").is_some());

        assert!(compile(r"\W").find(r"&").is_some());
    }

    #[test]
    fn test_digit_char_class() {
        assert!(compile(r"\d").find(r"0").is_some());
        assert!(compile(r"\d+").find(r"1234567890").is_some());

        assert!(compile(r"\D").find(r"a").is_some());
    }

    #[test]
    fn test_whitespace_char_class() {
        assert!(compile(r"\s").find(r" ").is_some());
        assert!(compile(r"\s+").find(" \n").is_some());

        assert!(compile(r"\S").find(r"a").is_some());
    }

    #[test]
    fn test_basic_character_class() {
        assert!(compile(r"[abc]").find(r"b").is_some());
        assert!(compile(r"[a-z]").find(r"x").is_some());
        assert!(compile(r"[a-zA-Z]").find(r"Y").is_some());

        assert!(compile(r"[abc]").find(r"d").is_none());
        assert!(compile(r"[a-z]").find(r"X").is_none());
        assert!(compile(r"[a-zA-Z]").find(r"5").is_none());
    }

    #[test]
    fn test_basic_negative_character_class() {
        assert!(compile(r"[^abc]").find(r"d").is_some());
        assert!(compile(r"[^a-z]").find(r"A").is_some());
        assert!(compile(r"[^a-zA-Z]").find(r"5").is_some());

        println!("{:?}", compile(r"[^a-zA-Z]"));
        println!("{:?}", compile(r"[^abc]").find(r"c"));
        assert!(compile(r"[^abc]").find(r"c").is_none());
        assert!(compile(r"[^a-z]").find(r"a").is_none());
        assert!(compile(r"[^a-zA-Z]").find(r"X").is_none());
    }

    #[test]
    fn test_quantified_character_class() {
        assert!(compile(r"[abc]+").find(r"abcabccba").is_some());
        assert!(compile(r"[^abc]+").find(r"def").is_some());
        assert!(compile(r"[a-z]*").find(r"a").is_some());
        assert!(compile(r"[^a-z]?").find(r"A").is_some());
        assert!(compile(r"[a-zA-Z]+").find(r"abcdAXZ").is_some());
        assert!(compile(r"[^a-zA-Z]*").find(r"52787&^%$").is_some());

        assert!(compile(r"[abc]+").find(r"defdfk").is_none());
        assert!(compile(r"[^abc]+").find(r"abc").is_none());
        assert!(compile(r"[a-z]+").find(r"ABC").is_none());
        assert!(compile(r"[^a-z]+").find(r"abc").is_none());
        assert!(compile(r"[a-zA-Z]+").find(r"1203845").is_none());
        assert!(compile(r"[^a-zA-Z]+").find(r"abcACCD").is_none());
    }
}
