use crate::gex::{GexMachine, Next, Rule, State};
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
                Rule::Range(character as u32, character as u32),
                Next::Target(2),
            )]),
            State::accept_state(),
        ],
    };
}

fn wildcard_machine() -> GexMachine {
    return GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(Rule::Range(0, 0x10ffff), Next::Target(2))]),
            State::accept_state(),
        ],
    };
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
        machine.states[1].push((Rule::Range('_' as u32, '_' as u32), Next::Target(2)));
    }

    machine
}

fn digit_char_machine(positive: bool) -> GexMachine {
    return GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(Rule::IsDigit(positive), Next::Target(2))]),
            State::accept_state(),
        ],
    };
}

fn whitespace_char_machine(positive: bool) -> GexMachine {
    return GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(Rule::IsWhitespace(positive), Next::Target(2))]),
            State::accept_state(),
        ],
    };
}

// NOTE: maybe it would have been easier to figure out token/astnode type layout by writing this
// first??
pub fn compile(input: &str) -> GexMachine {
    let tokens = tokenize(input);
    let ast = Ast::from_tokens(tokens);

    let mut combination_stack: Vec<GexMachine> = Vec::with_capacity(2);

    // TODO:
    // - write method for converting Literal token to machine
    // -
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
                        _ => panic!("unimplemented"),
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
}
