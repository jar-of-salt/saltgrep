use crate::gex::simple_machines::{
    digit_char_machine, machine_for, machine_for_character, manual_character_class_machine,
    whitespace_char_machine, wildcard_machine, word_char_machine,
};
use crate::gex::GexMachine;
use crate::railroad::{Ast, AstNode, SyntaxError};
use crate::tokenize::{tokenize, CharacterClassType, LiteralType, QuantifierType, TokenizeError};
use std::io;

use std::error;
use std::fmt;

type Result<T> = std::result::Result<T, CompilerError>;

#[derive(Debug, Clone)]
pub enum CompilerError {
    LexicalError(TokenizeError),
    SyntaxError(SyntaxError),
    MissingOperand(String),
    Catastrophic(String),
}

impl error::Error for CompilerError {}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompilerError::LexicalError(terror) => write!(f, "Invalid Token: {}", terror),
            CompilerError::SyntaxError(serror) => write!(f, "Invalid Syntax: {}", serror),
            CompilerError::MissingOperand(msg) => write!(f, "Operand Missing: {}", msg),
            CompilerError::Catastrophic(msg) => write!(f, "Catastrophic Error: {}", msg),
        }
    }
}

impl From<CompilerError> for io::Error {
    fn from(err: CompilerError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, err)
    }
}

fn get_operand(info: &str, combination_stack: &mut Vec<GexMachine>) -> Result<GexMachine> {
    combination_stack
        .pop()
        .ok_or_else(|| CompilerError::MissingOperand(info.to_string()))
}

// NOTE: maybe it would have been easier to figure out token/astnode type layout by writing this
// first??
pub fn compile(input: &str) -> Result<GexMachine> {
    let tokens = tokenize(input).or_else(|err| Err(CompilerError::LexicalError(err)))?;
    // TODO: error handling

    let ast = Ast::from_tokens(tokens).or_else(|err| Err(CompilerError::SyntaxError(err)))?;

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
                // TODO: determine if this panic is necessary
                LiteralType::EmptyString => panic!("Empty string not implemented"),
            },
            AstNode::Quantifier(qtype, _) => match qtype {
                QuantifierType::ZeroOrMore => {
                    let operand = get_operand("'*' (zero or more)", &mut combination_stack)?;
                    combination_stack.push(operand.zero_or_more());
                }
                QuantifierType::OneOrMore => {
                    let operand = get_operand("'+' (one or more)", &mut combination_stack)?;
                    combination_stack.push(operand.one_or_more());
                }
                QuantifierType::ZeroOrOne => {
                    let operand = get_operand("'?' (zero or one)", &mut combination_stack)?;
                    combination_stack.push(operand.zero_or_one());
                }
            },
            AstNode::Cons(_, _) => {
                let right = get_operand("'cons' (right hand side)", &mut combination_stack)?;
                let left = get_operand("'cons' (left hand side)", &mut combination_stack)?;
                combination_stack.push(left.cons(right));
            }
            AstNode::Alternation(_, _) => {
                let right =
                    get_operand("'|' (alternation right hand side)", &mut combination_stack)?;
                let left = get_operand("'|' (alternation left hand side)", &mut combination_stack)?;
                combination_stack.push(left.or(right));
            }
            AstNode::Group(_) => {
                let operand = get_operand("'()' (grouping)", &mut combination_stack)?;
                combination_stack.push(operand.group());
            }
        }
    }
    combination_stack
        .pop()
        .ok_or_else(|| CompilerError::Catastrophic("No NFA created".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::{Match, Matcher};

    macro_rules! assert_match {
        ($pattern:expr, $input:expr, $match_string:expr) => {
            let gex_machine = compile($pattern).unwrap();
            let result = gex_machine.find($input);

            assert!(result.is_some());
            let result_match = result.unwrap();
            assert_eq!($match_string, &$input[result_match.start..result_match.end]);
        };
    }

    macro_rules! assert_full_match {
        ($pattern:expr, $input:expr) => {
            assert_match!($pattern, $input, $input)
        };
    }

    macro_rules! assert_no_match {
        ($pattern:expr, $input:expr) => {
            let gex_machine = compile($pattern).unwrap();
            let result = gex_machine.find($input);

            assert!(result.is_none());
        };
    }

    macro_rules! assert_capture_match {
        ($captures:expr, $idx:expr, $start:expr, $end:expr) => {
            assert_eq!(
                *$captures.get(&$idx).unwrap(),
                Match {
                    start: $start,
                    end: $end
                }
            );
        };
    }

    macro_rules! assert_captures {
        ($pattern:expr, $input:expr, $(($idx:expr, $start:expr, $end:expr)),+) => {
            let machine = compile($pattern).unwrap();

            let wrapped_captures = machine.captures($input);
            assert!(wrapped_captures.is_some());
            let captures = wrapped_captures.unwrap();
            $(
                assert_capture_match!(captures, $idx, $start, $end);
            )*

            assert_eq!(captures.len(), [$( $idx ),*].len());
        };
    }

    #[test]
    fn test_nfa() {
        assert_full_match!(r"abcd+(efg)|i", r"i");
    }

    #[test]
    fn test_exotic_cases() {
        assert_full_match!(r"ab\c.d+(efg)|i", r"abcxdddefg");
    }

    #[test]
    fn test_word_char_class() {
        assert_full_match!(r"\w", r"a");
        assert_full_match!(r"\w+", r"abfhkg10235_1204");

        assert_no_match!(r"\w", r"-");
        assert_no_match!(r"\w+", r"%^$//-");

        assert_full_match!(r"\W", r"&");
        assert_match!(r"\W+", r"%^$//_0-", r"%^$//");

        assert_no_match!(r"\W", r"a");
        assert_no_match!(r"\W+", r"abckjdjfk");
    }

    #[test]
    fn test_digit_char_class() {
        assert_full_match!(r"\d", r"0");
        assert_full_match!(r"\d+", r"1234567890");

        assert_no_match!(r"\d", r"^");
        assert_no_match!(r"\d+", r"abc(*");

        assert_full_match!(r"\D", r"a");
        assert_full_match!(r"\D+", r"cddfi*&^w");

        assert_no_match!(r"\D", r"1");
        assert_no_match!(r"\D+", r"12345");
    }

    #[test]
    fn test_whitespace_char_class() {
        assert_full_match!(r"\s", r" ");
        assert_full_match!(r"\s+", " \n");

        assert_no_match!(r"\s", r"d");
        assert_no_match!(r"\s+", r"abc(*");

        assert_full_match!(r"\S", r"d");
        assert_full_match!(r"\S+", r"cddfi*&^w");

        assert_no_match!(r"\S", "\n");
        assert_no_match!(r"\S+", "  \n  ");
    }

    #[test]
    fn test_basic_character_class() {
        assert_full_match!(r"[abc]", r"b");
        assert_full_match!(r"[a-z]", r"x");
        assert_full_match!(r"[a-zA-Z]", r"Y");

        assert_no_match!(r"[abc]", r"d");
        assert_no_match!(r"[a-z]", r"X");
        assert_no_match!(r"[a-zA-Z]", r"5");
    }

    #[test]
    fn test_basic_negative_character_class() {
        assert_full_match!(r"[^abc]", r"d");
        assert_full_match!(r"[^a-z]", r"A");
        assert_full_match!(r"[^a-zA-Z]", r"5");

        assert_no_match!(r"[^abc]", r"c");
        assert_no_match!(r"[^a-z]", r"a");
        assert_no_match!(r"[^a-zA-Z]", r"X");
    }

    #[test]
    fn test_quantified_character_class() {
        assert_full_match!(r"[abc]+", r"abcabccba");
        assert_full_match!(r"[^abc]+", r"def");
        assert_full_match!(r"[a-z]*", r"a");
        assert_full_match!(r"[^a-z]?", r"A");
        assert_full_match!(r"[a-zA-Z]+", r"abcdAXZ");
        assert_full_match!(r"[^a-zA-Z]*", r"52787&^%$");

        assert_no_match!(r"[abc]+", r"defdfk");
        assert_no_match!(r"[^abc]+", r"abc");
        assert_no_match!(r"[a-z]+", r"ABC");
        assert_no_match!(r"[^a-z]+", r"abc");
        assert_no_match!(r"[a-zA-Z]+", r"1203845");
        assert_no_match!(r"[^a-zA-Z]+", r"abcACCD");
    }

    #[test]
    fn test_wildcard_matches() {
        assert_match!(r".*d", "mod", "mod");
        assert_match!(r".*d", "my mod in rust", "mod");
    }

    #[test]
    fn test_simple_capturing_group() {
        println!("{:?}", compile(r"(abc)").unwrap().captures(r"123abc456"));
        assert_captures!(r"(abc)", r"cdeabcdef", (0, 3, 6), (1, 3, 6));
    }

    #[test]
    fn test_simple_staggered_capturing_group() {
        assert_captures!(r"123(abc)", r"123abcdfdefg", (0, 0, 6), (1, 3, 6));
    }

    #[test]
    fn test_capturing_group() {
        assert_captures!(
            r"(abc)df(defg)(123)",
            r"abcdfdefg123",
            (0, 0, 12),
            (1, 0, 3),
            (2, 5, 9),
            (3, 9, 12)
        );
    }

    #[test]
    fn test_capturing_group_with_alternation1() {
        assert_captures!(
            r"(abc)df(defg)|(123)",
            r"abcdfdefg123",
            (0, 0, 9),
            (1, 0, 3),
            (2, 5, 9)
        );
    }

    #[test]
    fn test_capturing_group_with_alternation2() {
        assert_captures!(
            r"(abc)df(defg)|(123)",
            r"123abcdfdefg",
            (0, 0, 3),
            (3, 0, 3)
        );
    }

    #[test]
    fn test_capturing_group_with_alternation3() {
        assert_captures!(
            r"(abc)df(defg)|(1(23)a)",
            r"123abcdfdefg",
            (0, 0, 4),
            (3, 0, 4),
            (4, 1, 3)
        );
    }

    #[test]
    fn test_nested_capturing_group() {
        assert_captures!(
            r"(a(bc(de)))df(defg)",
            r"abcdedfdefgh",
            (0, 0, 11),
            (1, 0, 5),
            (2, 1, 5),
            (3, 3, 5),
            (4, 7, 11)
        );
    }

    #[test]
    fn test_empty_group() {
        assert_captures!(
            r"()af(())d(f()f)",
            r"afdffdiui",
            (0, 0, 5),
            (1, 0, 0),
            (2, 2, 2),
            (3, 2, 2),
            (4, 3, 5),
            (5, 4, 4)
        );
    }
}
