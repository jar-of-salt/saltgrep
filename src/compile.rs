use crate::gex::simple_machines::{
    digit_char_machine, machine_for, machine_for_character, manual_character_class_machine,
    whitespace_char_machine, wildcard_machine, word_char_machine,
};
use crate::gex::GexMachine;
use crate::railroad::{Ast, AstNode};
use crate::tokenize::{tokenize, CharacterClassType, LiteralType, QuantifierType};

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
        assert_no_match(r"\w+", r"%^$//-");

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

        assert_eq!(*captures.get(&1).unwrap(), Match { start: 0, end: 3 });
        assert_eq!(captures.len(), 2);
    }

    #[test]
    fn test_simple_staggered_capturing_group() {
        let machine = compile(r"123(abc)");
        let wrapped_captures = machine.captures(r"123abcdfdefg");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert_eq!(*captures.get(&1).unwrap(), Match { start: 3, end: 6 });
        assert_eq!(captures.len(), 2);
    }

    #[test]
    fn test_capturing_group() {
        assert_full_match(r"(abc)df(defg)(123)", r"abcdfdefg123");

        let machine = compile(r"(abc)df(defg)(123)");

        let wrapped_captures = machine.captures(r"abcdfdefg123");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert_eq!(*captures.get(&1).unwrap(), Match { start: 0, end: 3 });
        assert_eq!(*captures.get(&2).unwrap(), Match { start: 5, end: 9 });
        assert_eq!(*captures.get(&3).unwrap(), Match { start: 9, end: 12 });
        assert_eq!(captures.len(), 4);
    }

    #[test]
    fn test_capturing_group_with_alternation1() {
        let machine = compile(r"(abc)df(defg)|(123)");

        let wrapped_captures = machine.captures(r"abcdfdefg123");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert_eq!(*captures.get(&0).unwrap(), Match { start: 0, end: 9 });
        assert_eq!(*captures.get(&1).unwrap(), Match { start: 0, end: 3 });
        assert_eq!(*captures.get(&2).unwrap(), Match { start: 5, end: 9 });
        assert_eq!(captures.len(), 3);
    }

    #[test]
    fn test_capturing_group_with_alternation2() {
        let machine = compile(r"(abc)df(defg)|(123)");

        // TODO: implement similar method to `assert full match` for captures
        // TODO: match RHS of alternation
        let wrapped_captures = machine.captures(r"123abcdfdefg");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert_eq!(*captures.get(&0).unwrap(), Match { start: 0, end: 3 });
        assert_eq!(*captures.get(&3).unwrap(), Match { start: 0, end: 3 });
        assert_eq!(captures.len(), 2);
    }

    #[test]
    fn test_capturing_group_with_alternation3() {
        let machine = compile(r"(abc)df(defg)|(1(23)a)");

        // TODO: implement similar method to `assert full match` for captures
        // TODO: match RHS of alternation
        let wrapped_captures = machine.captures(r"123abcdfdefg");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert_eq!(*captures.get(&0).unwrap(), Match { start: 0, end: 4 });
        assert_eq!(*captures.get(&3).unwrap(), Match { start: 0, end: 4 });
        assert_eq!(*captures.get(&4).unwrap(), Match { start: 1, end: 3 });
        assert_eq!(captures.len(), 3);
    }

    #[test]
    fn test_nested_capturing_group() {
        let machine = compile(r"(a(bc(de)))df(defg)");

        let wrapped_captures = machine.captures(r"abcdedfdefgh");

        assert!(wrapped_captures.is_some());

        let captures = wrapped_captures.unwrap();

        assert_eq!(*captures.get(&0).unwrap(), Match { start: 0, end: 11 });
        assert_eq!(*captures.get(&1).unwrap(), Match { start: 0, end: 5 });
        assert_eq!(*captures.get(&2).unwrap(), Match { start: 1, end: 5 });
        assert_eq!(*captures.get(&3).unwrap(), Match { start: 3, end: 5 });
        assert_eq!(*captures.get(&4).unwrap(), Match { start: 7, end: 11 });
        assert_eq!(captures.len(), 5);
    }

    #[test]
    fn test_empty_group() {
        let machine = compile(r"()af(())d(f()f)");

        let wrapped_captures = machine.captures(r"afdffdiui");
        assert!(wrapped_captures.is_some());
        let captures = wrapped_captures.unwrap();
        assert_eq!(*captures.get(&0).unwrap(), Match { start: 0, end: 5 });
        assert_eq!(*captures.get(&1).unwrap(), Match { start: 0, end: 0 });
        assert_eq!(*captures.get(&2).unwrap(), Match { start: 2, end: 2 });
        assert_eq!(*captures.get(&3).unwrap(), Match { start: 2, end: 2 });
        assert_eq!(*captures.get(&4).unwrap(), Match { start: 3, end: 5 });
        assert_eq!(*captures.get(&5).unwrap(), Match { start: 4, end: 4 });
        assert_eq!(captures.len(), 6);
    }
}
