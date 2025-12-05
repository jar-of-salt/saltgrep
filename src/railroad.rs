use crate::tokenize::{Arity, Token};

#[derive(Debug, PartialEq, Eq)]
pub struct AstRef(u32);

#[derive(Debug, PartialEq, Eq)]
pub enum AstNode {
    Alternation(AstRef, AstRef),
    Cons(AstRef, AstRef),
    Group(AstRef),
    Quantifier(Token, AstRef),
    Literal(Token),
}

#[derive(Debug, PartialEq)]
pub struct Ast(Vec<AstNode>);

impl ToString for Ast {
    fn to_string(&self) -> String {
        let mut pretty = String::with_capacity(self.0.len() * 1.2 as usize);
        let length = self.0.len();
        for (idx, node) in self.0.iter().enumerate() {
            match node {
                AstNode::Alternation(_, _) => pretty.push('|'),
                AstNode::Cons(_, _) => pretty.push('J'),
                AstNode::Group(_) => pretty.push('G'),
                AstNode::Quantifier(token, _) => match token {
                    Token::ZeroOrMore => pretty.push('*'),
                    Token::OneOrMore => pretty.push('+'),
                    Token::ZeroOrOne => pretty.push('?'),
                    _ => panic!("No such quantifier: {:?}", token),
                },
                // TODO implement character classes
                AstNode::Literal(Token::Character(pos)) => {
                    pretty.push_str(pos.to_string().as_str())
                }
                _ => panic!("Unknown case: {:?}", node),
            }
            if idx < length - 1 {
                pretty.push(' ');
            }
        }
        pretty
    }
}

impl Ast {
    pub fn add(&mut self, node: AstNode) -> AstRef {
        let idx = self.0.len();
        self.0.push(node);
        AstRef(idx.try_into().expect("too many nodes in the AST"))
    }

    pub fn get(&self, node_ref: AstRef) -> &AstNode {
        &self.0[node_ref.0 as usize]
    }

    pub fn pprint(&self) {
        println!("{}", self.to_string());
    }

    pub fn from_tokens(tokens: Vec<Token>) -> Ast {
        let mut ast = Ast(Vec::with_capacity(2 * tokens.len()));
        let mut out_stack = Vec::<AstRef>::with_capacity(tokens.len());
        let mut op_stack = Vec::with_capacity(tokens.len() / 2);

        for token in tokens {
            match token {
                // when token is a character, or character-like object, push to output
                Token::Character(_) | Token::EscapedCharacter(_) | Token::CharacterSet(_, _, _) => {
                    out_stack.push(ast.add(AstNode::Literal(token)));
                }
                // when a group opens, push to operators
                Token::OpenGroup => {
                    op_stack.push(token);
                }
                // when a group closes, greedily consume the operator stack
                Token::CloseGroup => {
                    // TODO: add group position
                    add_group(0, &mut ast, &mut op_stack, &mut out_stack);
                }
                // Quantifiers are tightly bound, no op-stack nonsense for them, always bind
                // immediately
                Token::ZeroOrMore | Token::OneOrMore | Token::ZeroOrOne => {
                    let new_ref = ast.add(AstNode::Quantifier(
                        token,
                        out_stack.pop().expect("No operand found for quantifier"),
                    ));
                    out_stack.push(new_ref);
                }
                // Handle all other operations
                op_token => {
                    while let Some(previous_op) = op_stack.last() {
                        // Stop consuming if the previous operation is lower precedence than this one
                        if op_token.precedes(previous_op)
                            && !op_token.same_precedence_as(previous_op)
                        {
                            break;
                        }
                        let popped_op = op_stack.pop().unwrap();
                        let new_ref = ast.add(get_operator_node(popped_op, &mut out_stack));
                        out_stack.push(new_ref);
                    }
                    op_stack.push(op_token);
                }
            }
        }

        // issue is handling of the alternation/cons on the op stack

        while let Some(operation) = op_stack.pop() {
            let new_ref = ast.add(get_operator_node(operation, &mut out_stack));
            out_stack.push(new_ref);
        }

        ast
    }

    /// Alias for Ast::from_tokens.
    ///
    /// Reference to Shunting-Yard Algorithm.
    pub fn railroad(tokens: Vec<Token>) -> Ast {
        Ast::from_tokens(tokens)
    }
}

fn get_binary_operands(out_stack: &mut Vec<AstRef>) -> (AstRef, AstRef) {
    let right = out_stack.pop().expect("No RHS for operator");
    let left = out_stack.pop().expect("No LHS for operator");
    (left, right)
}

fn get_unary_operands(out_stack: &mut Vec<AstRef>) -> AstRef {
    out_stack.pop().expect("No argument for operator")
}

fn get_operator_node(op_token: Token, out_stack: &mut Vec<AstRef>) -> AstNode {
    match op_token.arity() {
        Arity::Binary => {
            let (left, right) = get_binary_operands(out_stack);
            match op_token {
                Token::Cons => AstNode::Cons(left, right),
                Token::Or => AstNode::Alternation(left, right),
                _ => panic!("Unknown Binary Operator: {:?}", op_token),
            }
        }
        Arity::Unary => {
            let arg = get_unary_operands(out_stack);
            match op_token {
                Token::ZeroOrMore | Token::OneOrMore | Token::ZeroOrOne => {
                    AstNode::Quantifier(op_token, arg)
                }
                Token::CloseGroup => AstNode::Group(arg),
                Token::OpenGroup => panic!("Unclosed OpenGroup token encountered"),
                _ => panic!("Unknown Unary Operator: {:?}", op_token),
            }
        }
        Arity::NoOp => panic!(
            "Can't convert non-operator token to operator node: {:?}",
            op_token
        ),
    }
}

fn add_group(
    group_pos: usize,
    ast: &mut Ast,
    op_stack: &mut Vec<Token>,
    out_stack: &mut Vec<AstRef>,
) {
    loop {
        let op_token = op_stack
            .pop()
            .unwrap_or_else(|| panic!("Unmatched group closure at {}", group_pos));
        if let Token::OpenGroup = op_token {
            break;
        }
        let new_ref = ast.add(get_operator_node(op_token, out_stack));
        out_stack.push(new_ref);
    }
    let group_contents = out_stack
        .pop()
        .unwrap_or_else(|| panic!("Nothing to group at {}", group_pos));
    let new_ref = ast.add(AstNode::Group(group_contents));
    out_stack.push(new_ref);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pointless_equivalence() {
        let tokens = vec![
            Token::Character(1),
            Token::Or,
            Token::Character(2),
            Token::Or,
            Token::Character(3),
        ];
        assert_eq!(
            Ast::from_tokens(tokens.clone()),
            Ast::railroad(tokens.clone())
        );
    }

    #[test]
    fn test_alternation_associativity() {
        // Pseudo-pattern: 1|2|3
        let tokens = vec![
            Token::Character(1),
            Token::Or,
            Token::Character(2),
            Token::Or,
            Token::Character(3),
        ];

        assert_eq!(
            vec![
                AstNode::Literal(Token::Character(1)),
                AstNode::Literal(Token::Character(2)),
                AstNode::Alternation(AstRef(0), AstRef(1)),
                AstNode::Literal(Token::Character(3)),
                AstNode::Alternation(AstRef(2), AstRef(3))
            ],
            Ast::railroad(tokens).0
        );
    }

    #[test]
    fn test_cons_associativity() {
        // Pseudo-pattern: 1|2|3
        let tokens = vec![
            Token::Character(1),
            Token::Cons,
            Token::Character(2),
            Token::Cons,
            Token::Character(3),
        ];

        assert_eq!(
            vec![
                AstNode::Literal(Token::Character(1)),
                AstNode::Literal(Token::Character(2)),
                AstNode::Cons(AstRef(0), AstRef(1)),
                AstNode::Literal(Token::Character(3)),
                AstNode::Cons(AstRef(2), AstRef(3))
            ],
            Ast::from_tokens(tokens).0
        );
    }

    #[test]
    fn test_mixed_pattern_1_for_ast() {
        // Pseudo-pattern: (12+34)5|6*
        let tokens = vec![
            Token::OpenGroup,
            Token::Character(1),
            Token::Cons,
            Token::Character(2),
            Token::OneOrMore,
            Token::Cons,
            Token::Character(3),
            Token::Cons,
            Token::Character(4),
            Token::CloseGroup,
            Token::Cons,
            Token::Character(5),
            Token::Or,
            Token::Character(6),
            Token::ZeroOrMore,
        ];

        assert_eq!(
            vec![
                AstNode::Literal(Token::Character(1)),
                AstNode::Literal(Token::Character(2)),
                AstNode::Quantifier(Token::OneOrMore, AstRef(1)),
                AstNode::Cons(AstRef(0), AstRef(2)),
                AstNode::Literal(Token::Character(3)),
                AstNode::Cons(AstRef(3), AstRef(4)),
                AstNode::Literal(Token::Character(4)),
                AstNode::Cons(AstRef(5), AstRef(6)),
                AstNode::Group(AstRef(7)),
                AstNode::Literal(Token::Character(5)),
                AstNode::Cons(AstRef(8), AstRef(9)),
                AstNode::Literal(Token::Character(6)),
                AstNode::Quantifier(Token::ZeroOrMore, AstRef(11)),
                AstNode::Alternation(AstRef(10), AstRef(12))
            ],
            Ast::from_tokens(tokens).0
        );
    }

    #[test]
    fn test_mixed_pattern_2_for_ast_and_rpn() {
        // Pseudo-pattern: (12+(34)*5(6(7)8))9?|(10(11(12|13|14)))
        let tokens = vec![
            Token::OpenGroup,
            Token::Character(1),
            Token::Cons,
            Token::Character(2),
            Token::OneOrMore,
            Token::Cons,
            Token::OpenGroup,
            Token::Character(3),
            Token::Cons,
            Token::Character(4),
            Token::CloseGroup,
            Token::ZeroOrMore,
            Token::Cons,
            Token::Character(5),
            Token::Cons,
            Token::OpenGroup,
            Token::Character(6),
            Token::Cons,
            Token::OpenGroup,
            Token::Character(7),
            Token::CloseGroup,
            Token::Cons,
            Token::Character(8),
            Token::CloseGroup,
            Token::CloseGroup,
            Token::Cons,
            Token::Character(9),
            Token::ZeroOrOne,
            Token::Or,
            Token::OpenGroup,
            Token::Character(10),
            Token::Cons,
            Token::OpenGroup,
            Token::Character(11),
            Token::Cons,
            Token::OpenGroup,
            Token::Character(12),
            Token::Or,
            Token::Character(13),
            Token::Or,
            Token::Character(14),
            Token::CloseGroup,
            Token::CloseGroup,
            Token::CloseGroup,
        ];

        // Test RPN
        assert_eq!(
            "1 2 + J 3 4 J G * J 5 J 6 7 G J 8 J G J G 9 ? J 10 11 12 13 | 14 | G J G J G |",
            Ast::from_tokens(tokens.clone()).to_string()
        );

        assert_eq!(
            vec![
                AstNode::Literal(Token::Character(1)),
                AstNode::Literal(Token::Character(2)),
                AstNode::Quantifier(Token::OneOrMore, AstRef(1)),
                AstNode::Cons(AstRef(0), AstRef(2)),
                AstNode::Literal(Token::Character(3)),
                AstNode::Literal(Token::Character(4)),
                AstNode::Cons(AstRef(4), AstRef(5)),
                AstNode::Group(AstRef(6)),
                AstNode::Quantifier(Token::ZeroOrMore, AstRef(7)),
                AstNode::Cons(AstRef(3), AstRef(8)),
                AstNode::Literal(Token::Character(5)),
                AstNode::Cons(AstRef(9), AstRef(10)),
                AstNode::Literal(Token::Character(6)),
                AstNode::Literal(Token::Character(7)),
                AstNode::Group(AstRef(13)),
                AstNode::Cons(AstRef(12), AstRef(14)),
                AstNode::Literal(Token::Character(8)),
                AstNode::Cons(AstRef(15), AstRef(16)),
                AstNode::Group(AstRef(17)),
                AstNode::Cons(AstRef(11), AstRef(18)),
                AstNode::Group(AstRef(19)),
                AstNode::Literal(Token::Character(9)),
                AstNode::Quantifier(Token::ZeroOrOne, AstRef(21)),
                AstNode::Cons(AstRef(20), AstRef(22)),
                AstNode::Literal(Token::Character(10)),
                AstNode::Literal(Token::Character(11)),
                AstNode::Literal(Token::Character(12)),
                AstNode::Literal(Token::Character(13)),
                AstNode::Alternation(AstRef(26), AstRef(27)),
                AstNode::Literal(Token::Character(14)),
                AstNode::Alternation(AstRef(28), AstRef(29)),
                AstNode::Group(AstRef(30)),
                AstNode::Cons(AstRef(25), AstRef(31)),
                AstNode::Group(AstRef(32)),
                AstNode::Cons(AstRef(24), AstRef(33)),
                AstNode::Group(AstRef(34)),
                AstNode::Alternation(AstRef(23), AstRef(35))
            ],
            Ast::from_tokens(tokens).0
        );
    }
}
