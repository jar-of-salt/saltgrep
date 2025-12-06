// todo: tokenizer
// input pattern -> tokens -> parse? into NFA
// TODO: visualize!
use std::ops::Range;

// If I want to try supporting UTF-16 etc.
const CHAR_WIDTH: usize = 1;

fn shift_chars(position: usize, shift: usize) -> usize {
    position + shift * CHAR_WIDTH
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QuantifierType {
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum LiteralType {
    // TODO: implement wildcard
    Wildcard,
    Character,
    EscapedCharacter,
    CharacterClass(CharacterClassType, bool),
    EmptyString,
}

// TODO: implement other character classes
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CharacterClassType {
    Manual,
    Whitespace,
    Digit,
    Word,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TokenType {
    Quantifier(QuantifierType),
    Alternation,
    Cons,
    Literal(LiteralType),
    OpenGroup,
    CloseGroup,
}

// pub mod BitFlags {
//     pub const INVERT_FLAG_MASK = 0x00;
//     pub const INVERT_FLAG_TRUE = 0x01;
// }

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Token {
    pub kind: TokenType,
    pub position: (usize, usize),
    pub flags: u8,
}

fn should_join_literals(token: &Token) -> bool {
    match &token.kind {
        TokenType::CloseGroup | TokenType::Quantifier(_) | TokenType::Literal(_) => true,
        _ => false,
    }
}

pub enum Arity {
    Binary,
    Unary,
    NoOp,
}

impl Token {
    fn create_long(kind: TokenType, start_position: usize, end_position: usize) -> Self {
        Token {
            kind,
            position: (start_position, end_position),
            flags: 0u8,
        }
    }

    fn create(kind: TokenType, position: usize) -> Self {
        Token::create_long(kind, position, position + 1)
    }

    fn cons(position: usize) -> Self {
        Token::create_long(TokenType::Cons, position, position)
    }

    fn quantifier(kind: QuantifierType, position: usize) -> Self {
        Token::create(TokenType::Quantifier(kind), position)
    }

    fn open_group(position: usize) -> Self {
        Token::create(TokenType::OpenGroup, position)
    }

    fn close_group(position: usize) -> Self {
        Token::create(TokenType::CloseGroup, position)
    }

    pub fn input_range(&self) -> Range<usize> {
        self.position.0..self.position.1
    }

    pub fn empty_string(position: usize) -> Self {
        Token {
            kind: TokenType::Literal(LiteralType::EmptyString),
            position: (position, position),
            flags: 0x0,
        }
    }

    pub fn start(&self) -> usize {
        self.position.0
    }

    pub fn end(&self) -> usize {
        self.position.1
    }

    fn get_precedence(&self) -> i8 {
        -(match &self.kind {
            // Token::LeftSquareBracket | Token::RightSquareBracket => 3,
            // TODO: OpenGroup might not need to be here; aka should not get popped from op stack
            // if lower priority operator is encountered
            TokenType::CloseGroup => 4,
            TokenType::Quantifier(_) => 5,
            TokenType::Cons => 6,
            TokenType::Alternation => 8,
            TokenType::OpenGroup => 10,
            TokenType::Literal(_) => 0,
        })
    }

    pub fn precedes(&self, other: &Self) -> bool {
        self.get_precedence() > other.get_precedence()
    }

    pub fn same_precedence_as(&self, other: &Self) -> bool {
        self.get_precedence() == other.get_precedence()
    }

    pub fn arity(&self) -> Arity {
        match self.kind {
            TokenType::Quantifier(_) | TokenType::CloseGroup => Arity::Unary,
            TokenType::Cons | TokenType::Alternation => Arity::Binary,
            _ => Arity::NoOp,
        }
    }
}

// +---+----------------------------------------------------------+
// |   |             ERE Precedence (from high to low)            |
// +---+----------------------------------------------------------+
// | 1 | Collation-related bracket symbols | [==] [::] [..]       |
// | 2 | Escaped characters                | \<special character> |
// | 3 | Bracket expression                | []                   |
// | 4 | Grouping                          | ()                   |
// | 5 | Single-character-ERE duplication  | * + ? {m,n}          |
// | 6 | Concatenation                     |                      |
// | 7 | Anchoring                         | ^ $                  |
// | 8 | Alternation                       | |                    |
// +---+-----------------------------------+----------------------+

fn munch_character_class(input: &[u8], position: usize) -> (Token, usize) {
    let remaining_input = &input[position + 1..];
    let mut found: Option<usize> = None;
    // TODO: test case for failure where there is not another character
    for index in 0..remaining_input.len() {
        if &remaining_input[index..index + 1] == b"]" && &remaining_input[index - 1..index] != br"\"
        {
            found = Some(position + index + 1);
            break;
        }
    }

    if let Some(close_bracket_position) = found {
        let positive_match = &remaining_input[0..1] != b"^";
        let token = Token::create_long(
            TokenType::Literal(LiteralType::CharacterClass(
                CharacterClassType::Manual,
                positive_match,
            )),
            position,
            close_bracket_position + 1, // need the position AFTER the ] k
        );

        (token, close_bracket_position)
    } else {
        // TODO: add error reporting module
        panic!("Unterminated character set at {}", position);
    }
}

fn munch_character_class_escape(input: &[u8], position: usize) -> Option<Token> {
    let end_position = shift_chars(position, 2);
    match input {
        br"\s" | br"\S" => Some(Token::create_long(
            TokenType::Literal(LiteralType::CharacterClass(
                CharacterClassType::Whitespace,
                input == br"\s",
            )),
            position,
            end_position,
        )),
        br"\w" | br"\W" => Some(Token::create_long(
            TokenType::Literal(LiteralType::CharacterClass(
                CharacterClassType::Word,
                input == br"\w",
            )),
            position,
            end_position,
        )),
        br"\d" | br"\D" => Some(Token::create_long(
            TokenType::Literal(LiteralType::CharacterClass(
                CharacterClassType::Digit,
                input == br"\d",
            )),
            position,
            end_position,
        )),
        _ => None,
    }
}

fn munch_escape_character(position: usize) -> Token {
    // supports arbitrary escape characters, but also gives me flexibility to add word boundary
    // support in the future, etc. etc.
    Token::create_long(
        TokenType::Literal(LiteralType::EscapedCharacter),
        position,
        shift_chars(position, 2),
    )
}

// TODO: improve name; it inserts a cons if necessary
fn insert_cons(tokens: &mut Vec<Token>) {
    if let Some(token) = tokens.last() {
        if should_join_literals(token) {
            tokens.push(Token::cons(token.position.1));
        }
    }
}

fn munch_token(
    input: &[u8],
    character: &[u8],
    position: usize,
    tokens: &mut Vec<Token>,
) -> (Token, usize) {
    let mut new_position = position;
    (
        match character {
            b"(" => {
                insert_cons(tokens);
                Token::open_group(position)
            }
            b")" => Token::close_group(position),
            b"[" => {
                insert_cons(tokens);
                let (token, end_char_class) = munch_character_class(input, position);
                new_position = end_char_class;
                token
            }
            b"|" => Token::create(TokenType::Alternation, position),
            b"*" => Token::quantifier(QuantifierType::ZeroOrMore, position),
            b"+" => Token::quantifier(QuantifierType::OneOrMore, position),
            b"?" => Token::quantifier(QuantifierType::ZeroOrOne, position),
            br"\" => {
                new_position = shift_chars(position, 1);
                insert_cons(tokens);
                munch_character_class_escape(&input[position..new_position], position)
                    .unwrap_or(munch_escape_character(position))
            }
            _ => {
                insert_cons(tokens);
                Token::create(TokenType::Literal(LiteralType::Character), position)
            }
        },
        new_position,
    )
}

pub fn tokenize(input: &[u8]) -> Vec<Token> {
    let mut position = 0;
    let mut tokens = Vec::new();
    let num_chars = input.len();

    while position < num_chars {
        let (token, new_position) =
            munch_token(input, &input[position..position + 1], position, &mut tokens);
        position = new_position + 1;
        tokens.push(token);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "Unterminated character set at 3")]
    fn test_bad_character_class() {
        tokenize(br"123[fdhk\]dfsdf");
    }

    #[test]
    fn test_tokenize() {
        // pattern: `ab+`
        assert_eq!(
            vec![
                Token::create(TokenType::Literal(LiteralType::Character), 0),
                Token::cons(1),
                Token::create(TokenType::Literal(LiteralType::Character), 1),
                Token::cons(2),
                Token::create(TokenType::Literal(LiteralType::Character), 2),
                Token::cons(3),
                Token::create(TokenType::Literal(LiteralType::Character), 3),
                Token::cons(4),
                Token::create_long(
                    TokenType::Literal(LiteralType::CharacterClass(
                        CharacterClassType::Manual,
                        true
                    )),
                    4,
                    8
                ),
                Token::quantifier(QuantifierType::OneOrMore, 8),
                Token::cons(9),
                Token::create(TokenType::Literal(LiteralType::Character), 9),
                Token::quantifier(QuantifierType::ZeroOrMore, 10),
                Token::create(TokenType::Alternation, 11),
                Token::create(TokenType::Literal(LiteralType::Character), 12),
                Token::quantifier(QuantifierType::ZeroOrOne, 13),
                Token::cons(14),
                Token::create(TokenType::Literal(LiteralType::Character), 14),
                Token::cons(15),
                Token::create_long(TokenType::Literal(LiteralType::EscapedCharacter), 15, 17),
                Token::cons(17),
                Token::create(TokenType::Literal(LiteralType::Character), 17),
                Token::cons(18),
                Token::create_long(
                    TokenType::Literal(LiteralType::CharacterClass(
                        CharacterClassType::Manual,
                        false
                    ),),
                    18,
                    24
                ),
                Token::cons(24),
                Token::open_group(24),
                Token::create(TokenType::Literal(LiteralType::Character), 25),
                Token::cons(26),
                Token::create(TokenType::Literal(LiteralType::Character), 26),
                Token::cons(27),
                Token::create(TokenType::Literal(LiteralType::Character), 27),
                Token::cons(28),
                Token::create(TokenType::Literal(LiteralType::Character), 28),
                Token::close_group(29),
                Token::cons(30),
                Token::create(TokenType::Literal(LiteralType::Character), 30),
            ],
            tokenize(b"abce[fg]+h*|i?j\\kl[^a-c](abcd)i")
        )
    }
}
