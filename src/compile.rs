use crate::gex::{GexMachine, Next, Rule, State};
use crate::railroad::{Ast, AstNode};
use crate::tokenize::{tokenize, LiteralType, QuantifierType, Token, shift_chars};

fn machine_for(token: Token, input: &[u8]) -> GexMachine {
    let range_value = input[token.input_range()][0];
    return GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(
                Rule::Range(range_value, range_value),
                Next::Target(2),
            )]),
            State::accept_state(),
        ],
    };
}

fn machine_for_character(character: u8) -> GexMachine {
    return GexMachine {
        states: vec![
            State::from_transitions(vec![(Rule::Null, Next::Target(1))]),
            State::from_transitions(vec![(Rule::Range(character, character), Next::Target(2))]),
            State::accept_state(),
        ],
    };
}

fn machine_for_literal(literal_type: LiteralType, input: &[u8]) -> GexMachine {
    match literal_type {
        // TODO: evaluate for potential UTF-8 handling
        LiteralType::Character => machine_for_character(input[0]),
        LiteralType::EscapedCharacter => machien_for_character(input[1]);
    }
}

// NOTE: maybe it would have been easier to figure out token/astnode type layout by writing this
// first??
pub fn compile(input: &[u8]) -> GexMachine {
    let tokens = tokenize(input);
    let ast = Ast::from_tokens(tokens);

    let mut combination_stack: Vec<GexMachine> = Vec::with_capacity(2);

    // TODO:
    // - write method for converting Literal token to machine
    // -
    for ast_node in ast.0.iter() {
        match ast_node {
            AstNode::Literal(_ltype, token) => combination_stack.push(machine_for(*token, input)),
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
        assert!(compile(br"abcd+(efg)|i").find(br"i").is_some());
    }
}
