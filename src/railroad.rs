use crate::operators::{Arity, Operator};
use crate::tokenize::{LiteralType, QuantifierType, Token, TokenType};
use std::fmt;

type Result<T> = std::result::Result<T, SyntaxError>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SyntaxError {
    // UnclosedGroup(usize),
    // MissingBinaryOperand(TokenType, usize),
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // match self {
        // SyntaxError::EmptyCharacterSet(position) => {
        //     write!(f, "Empty character set at {}", position)
        // }
        // SyntaxError::UnterminatedCharacterSet(position) => {
        //     write!(f, "Unterminated character set at {}", position)
        // }
        // SyntaxError::UnterminatedEscape(position) => {
        //     write!(f, "Unterminated escape character at {}", position)
        // }
        // }
        write!(f, "")
    }
}

#[derive(Debug, PartialEq)]
pub struct AstRef(usize);

#[derive(Debug, PartialEq)]
pub enum AstNode {
    Alternation(AstRef, AstRef),
    Cons(AstRef, AstRef),
    Group(AstRef),
    Quantifier(QuantifierType, AstRef),
    Literal(LiteralType, Token),
}

// TODO: potential limitation of the implementation -> character classes are built at lex time instead
// of at parse time
//
// TODO: separate this so Tokens implement To<AstNode>? Just evaluate more separation of concerns

#[derive(Debug, PartialEq)]
pub struct Ast(pub Vec<AstNode>);

impl ToString for Ast {
    fn to_string(&self) -> String {
        let mut pretty = String::with_capacity(self.0.len() * 1.2 as usize);
        let length = self.0.len();
        for (idx, node) in self.0.iter().enumerate() {
            match node {
                AstNode::Alternation(_, _) => pretty.push('|'),
                AstNode::Cons(_, _) => {
                    pretty.push('J');
                }
                AstNode::Group(_) => pretty.push('G'),
                AstNode::Quantifier(qtype, _) => match qtype {
                    QuantifierType::ZeroOrMore => pretty.push('*'),
                    QuantifierType::OneOrMore => pretty.push('+'),
                    QuantifierType::ZeroOrOne => pretty.push('?'),
                },
                // TODO implement character classes
                AstNode::Literal(_, token) => {
                    pretty.push_str(format!("{}..{}", token.start(), token.end()).as_str())
                }
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
        self.0.push(node);
        AstRef(self.0.len())
    }

    pub fn get(&self, node_ref: AstRef) -> &AstNode {
        &self.0[node_ref.0 as usize]
    }

    pub fn size(&self) -> usize {
        self.0.len()
    }

    pub fn pprint(&self) {
        println!("{}", self.to_string());
    }

    pub fn from_tokens(tokens: Vec<Token>) -> Result<Ast> {
        let mut ast = Ast(Vec::with_capacity(2 * tokens.len()));
        let mut out_stack = Vec::<AstRef>::with_capacity(tokens.len());
        let mut op_stack = Vec::with_capacity(tokens.len() / 2);

        let mut peekable_tokens = tokens.into_iter().peekable();

        while let Some(token) = peekable_tokens.next() {
            match token.kind {
                // when token is a character, or character-like object, push to output
                TokenType::Literal(literal_type) => {
                    out_stack.push(ast.add(AstNode::Literal(literal_type, token)));
                }
                // when a group opens, push to operators
                TokenType::OpenGroup => {
                    op_stack.push(token);
                    if let Some(Token {
                        kind: TokenType::CloseGroup,
                        ..
                    }) = peekable_tokens.peek()
                    {
                        out_stack.push(ast.add(AstNode::Literal(
                            LiteralType::Character,
                            Token::empty_string(token.start() + 1),
                        )));
                    }
                }
                // when a group closes, greedily consume the operator stack
                TokenType::CloseGroup => {
                    add_group(token.start(), &mut ast, &mut op_stack, &mut out_stack);
                }
                // Quantifiers are tightly bound, no op-stack nonsense for them, always bind
                // immediately
                TokenType::Quantifier(quantifier_type) => {
                    let new_ref = ast.add(AstNode::Quantifier(
                        quantifier_type,
                        out_stack.pop().unwrap_or_else(|| {
                            panic!("No operand found for {:?} at {}", token.kind, token.start())
                        }),
                    ));
                    out_stack.push(new_ref);
                }
                // Handle all other operations
                _ => {
                    while let Some(previous_op) = op_stack.last() {
                        // Stop consuming if the previous operation is lower precedence than this one
                        if token.precedes(previous_op) && !token.same_precedence_as(previous_op) {
                            break;
                        }
                        let popped_op = op_stack.pop().unwrap();
                        let new_ref = ast.add(get_operator_node(popped_op, &mut out_stack));
                        out_stack.push(new_ref);
                    }
                    op_stack.push(token);
                }
            }
        }

        // issue is handling of the alternation/cons on the op stack

        while let Some(operation) = op_stack.pop() {
            let new_ref = ast.add(get_operator_node(operation, &mut out_stack));
            out_stack.push(new_ref);
        }

        Ok(ast)
    }

    /// Alias for Ast::from_tokens.
    ///
    /// Reference to Shunting-Yard Algorithm.
    pub fn railroad(tokens: Vec<Token>) -> Result<Ast> {
        Ast::from_tokens(tokens)
    }
}

fn get_binary_operands(out_stack: &mut Vec<AstRef>, position: usize) -> (AstRef, AstRef) {
    let right = out_stack
        .pop()
        .unwrap_or_else(|| panic!("No RHS for operator at {}", position));
    let left = out_stack
        .pop()
        .unwrap_or_else(|| panic!("No LHS for operator at {}", position));
    (left, right)
}

fn get_unary_operands(out_stack: &mut Vec<AstRef>, position: usize) -> AstRef {
    out_stack
        .pop()
        .unwrap_or_else(|| panic!("No operand for operator at {}", position))
}

fn get_operator_node(op_token: Token, out_stack: &mut Vec<AstRef>) -> AstNode {
    match op_token.arity() {
        Arity::Binary => {
            let (left, right) = get_binary_operands(out_stack, op_token.start());
            match op_token.kind {
                TokenType::Cons => AstNode::Cons(left, right),
                TokenType::Alternation => AstNode::Alternation(left, right),
                _ => panic!("Unknown Binary Operator: {:?}", op_token),
            }
        }
        Arity::Unary => {
            let arg = get_unary_operands(out_stack, op_token.start());
            match op_token.kind {
                TokenType::Quantifier(qtype) => AstNode::Quantifier(qtype, arg),
                TokenType::CloseGroup => AstNode::Group(arg),
                TokenType::OpenGroup => panic!("Unclosed OpenGroup token encountered"),
                _ => panic!(
                    "Unknown Unary Operator {:?} at {}",
                    op_token.kind,
                    op_token.start()
                ),
            }
        }
        Arity::NoOp => panic!(
            "Can't convert non-operator token at {} to operator node: {:?}",
            op_token.start(),
            op_token.kind
        ),
        _ => panic!("Unsupported operator arity at {}", op_token.start()),
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
        if let TokenType::OpenGroup = op_token.kind {
            break;
        }
        let new_ref = ast.add(get_operator_node(op_token, out_stack));
        out_stack.push(new_ref);
    }
    let group_contents = out_stack.pop().expect("Group must have contents");
    let new_ref = ast.add(AstNode::Group(group_contents));
    out_stack.push(new_ref);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tokenize;

    // TODO: expected message
    #[test]
    #[should_panic]
    fn test_begins_with_quantifier() {
        assert!(
            Ast::from_tokens(tokenize::tokenize("*abcd").expect("tokenization failed")).is_ok()
        );
    }

    // TODO: expected message
    #[test]
    #[should_panic]
    fn test_quantifier_on_open_group() {
        assert!(
            Ast::from_tokens(tokenize::tokenize("abcd(*abcd)").expect("tokenization failed"))
                .is_ok()
        );
    }

    // TODO: expected message
    #[test]
    #[should_panic]
    fn test_no_close_group() {
        assert!(Ast::from_tokens(
            tokenize::tokenize("abcd(*abcd(abcde?fg+)?").expect("tokenization failed")
        )
        .is_ok());
    }

    // TODO: expected message
    #[test]
    #[should_panic]
    fn test_no_open_group() {
        assert!(Ast::from_tokens(
            tokenize::tokenize("abcd*abcd(abcde)?fg+)?").expect("tokenization failed"),
        )
        .is_ok());
    }

    // TODO: make sure gex can match empty string
    #[test]
    fn test_empty_group() {
        assert!(Ast::from_tokens(tokenize::tokenize("()").expect("tokenization failed")).is_ok());
    }

    // TODO: expected message
    #[test]
    #[should_panic]
    fn test_double_alternation() {
        assert!(Ast::from_tokens(tokenize::tokenize("a||").expect("tokenization failed")).is_ok());
    }

    // TODO: expected message
    #[test]
    #[should_panic]
    fn test_no_rhs_alternation() {
        assert!(Ast::from_tokens(tokenize::tokenize("c|").expect("tokenization failed")).is_ok());
    }

    // TODO: expected message
    #[test]
    #[should_panic]
    fn test_no_lhs_alternation() {
        assert!(Ast::from_tokens(tokenize::tokenize("|c").expect("tokenization failed")).is_ok());
    }

    #[test]
    fn test_pointless_equivalence() {
        let tokens = tokenize::tokenize("a|b|c").expect("tokenization failed");
        assert_eq!(
            Ast::from_tokens(tokens.clone()).expect("parsing succeeds"),
            Ast::railroad(tokens.clone()).expect("parsing succeeds")
        );
    }

    #[test]
    fn test_alternation_associativity() {
        let tokens = tokenize::tokenize("a|b|c").expect("tokenization failed");

        assert_eq!(
            "0..1 2..3 | 4..5 |",
            Ast::railroad(tokens).unwrap().to_string()
        );
    }

    #[test]
    fn test_cons_associativity() {
        // Pseudo-pattern: 1|2|3
        let tokens = tokenize::tokenize("abc").expect("tokenization failed");

        assert_eq!(
            "0..1 1..2 J 2..3 J",
            Ast::from_tokens(tokens).unwrap().to_string()
        );
    }

    #[test]
    fn test_mixed_pattern_1() {
        // Pseudo-pattern: (12+34)5|6*
        let tokens = tokenize::tokenize("(ab+34)5|6*").expect("tokenization failed");

        assert_eq!(
            "1..2 2..3 + J 4..5 J 5..6 J G 7..8 J 9..10 * |",
            Ast::from_tokens(tokens).unwrap().to_string()
        );
    }

    #[test]
    fn test_mixed_pattern_2() {
        let tokens =
            tokenize::tokenize("(ab+(cd)*e(f(g)h))i?|(j(k(l|m|n)))").expect("tokenization failed");
        assert_eq!(
            "1..2 2..3 + J 5..6 6..7 J G * J 9..10 J 11..12 13..14 G J 15..16 J G J G 18..19 ? J 22..23 24..25 26..27 28..29 | 30..31 | G J G J G |",
            Ast::from_tokens(tokens).unwrap().to_string()
        );
    }
}
