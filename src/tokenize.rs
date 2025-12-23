// TODO: visualize!
use crate::operators::{Arity, Operator};
use std::fmt;
use std::iter::Peekable;
use std::ops::Range;
use std::str::Chars;

type Result<T> = std::result::Result<T, TokenizeError>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TokenizeError {
    EmptyCharacterSet(usize),
    UnterminatedCharacterSet(usize),
    UnterminatedEscape(usize),
}

impl fmt::Display for TokenizeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TokenizeError::EmptyCharacterSet(position) => {
                write!(f, "Empty character set at {}", position)
            }
            TokenizeError::UnterminatedCharacterSet(position) => {
                write!(f, "Unterminated character set at {}", position)
            }
            TokenizeError::UnterminatedEscape(position) => {
                write!(f, "Unterminated escape character at {}", position)
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QuantifierType {
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum LiteralType {
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
    /// Byte span of the token in the source string.
    pub position: (usize, usize),
    pub flags: u8,
}

fn should_join_literals(token: &Token) -> bool {
    match &token.kind {
        TokenType::CloseGroup | TokenType::Quantifier(_) | TokenType::Literal(_) => true,
        _ => false,
    }
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
}

impl Operator for Token {
    fn arity(&self) -> Arity {
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

fn munch_character_class(remaining_chars: &mut Peekable<Chars>, position: usize) -> Result<Token> {
    // TODO: evaluate this, as this choice might result in some WEIRD bugs
    let mut previous_char = '\0';
    let mut inverted = false;
    // start at first char

    let mut next_position = position + '['.len_utf8();

    // TODO: test case for failure where there is not another character
    if let Some(&'^') = remaining_chars.peek() {
        inverted = true;
        remaining_chars.next();
        next_position += '^'.len_utf8();
    }

    let mut num_chars_in_class = 0;

    for remaining_char in remaining_chars {
        if remaining_char == ']' && previous_char != '\\' {
            if num_chars_in_class == 0 {
                return Err(TokenizeError::EmptyCharacterSet(position)); //("Empty character set at {}", position);
            }
            let token = Token::create_long(
                TokenType::Literal(LiteralType::CharacterClass(
                    CharacterClassType::Manual,
                    !inverted,
                )),
                position,
                // TODO: is this the problem?
                next_position + remaining_char.len_utf8(),
            );

            return Ok(token);
        }
        num_chars_in_class += 1;
        next_position += remaining_char.len_utf8();
        previous_char = remaining_char;
    }

    // TODO: add error reporting module
    Err(TokenizeError::UnterminatedCharacterSet(position)) // !("Unterminated character set at {}", position);
}

fn munch_character_class_escape(
    remaining_chars: &mut Peekable<Chars>,
    position: usize,
) -> Result<Option<Token>> {
    // TODO: is this a good error? Should this error happen? Is an \ at the end of a string OK?
    let next_character = remaining_chars
        .peek()
        .ok_or(TokenizeError::UnterminatedEscape(position))?;

    let end_position = position + '\\'.len_utf8() + next_character.len_utf8();

    let character_class_escape_token = match next_character {
        's' | 'S' => Some(Token::create_long(
            TokenType::Literal(LiteralType::CharacterClass(
                CharacterClassType::Whitespace,
                *next_character == 's',
            )),
            position,
            end_position,
        )),
        'w' | 'W' => Some(Token::create_long(
            TokenType::Literal(LiteralType::CharacterClass(
                CharacterClassType::Word,
                *next_character == 'w',
            )),
            position,
            end_position,
        )),
        'd' | 'D' => Some(Token::create_long(
            TokenType::Literal(LiteralType::CharacterClass(
                CharacterClassType::Digit,
                *next_character == 'd',
            )),
            position,
            end_position,
        )),
        _ => None,
    };

    if let Some(_) = character_class_escape_token {
        remaining_chars.next();
    }
    Ok(character_class_escape_token)
}

fn munch_escape_character(remaining_chars: &mut Peekable<Chars>, position: usize) -> Result<Token> {
    // supports arbitrary escape characters, but also gives me flexibility to add word boundary
    // support in the future, etc. etc.
    remaining_chars
        .next()
        .map(|remaining_char| {
            Token::create_long(
                TokenType::Literal(LiteralType::EscapedCharacter),
                position,
                position + '\\'.len_utf8() + remaining_char.len_utf8(),
            )
        })
        .ok_or(TokenizeError::UnterminatedEscape(position))
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
    remaining_chars: &mut Peekable<Chars>,
    character: &char,
    position: usize,
    tokens: &mut Vec<Token>,
) -> Result<Token> {
    match character {
        '(' => {
            insert_cons(tokens);
            Ok(Token::open_group(position))
        }
        ')' => Ok(Token::close_group(position)),
        '[' => {
            insert_cons(tokens);
            munch_character_class(remaining_chars, position)
        }
        '|' => Ok(Token::create(TokenType::Alternation, position)),
        '*' => Ok(Token::quantifier(QuantifierType::ZeroOrMore, position)),
        '+' => Ok(Token::quantifier(QuantifierType::OneOrMore, position)),
        '?' => Ok(Token::quantifier(QuantifierType::ZeroOrOne, position)),
        '\\' => {
            insert_cons(tokens);
            munch_character_class_escape(remaining_chars, position)
                .transpose()
                .unwrap_or_else(|| munch_escape_character(remaining_chars, position))
        }
        '.' => {
            insert_cons(tokens);
            Ok(Token::create(
                TokenType::Literal(LiteralType::Wildcard),
                position,
            ))
        }
        _ => {
            insert_cons(tokens);
            Ok(Token::create(
                TokenType::Literal(LiteralType::Character),
                position,
            ))
        }
    }
}

// TODO: having a type Tokenization supporting .add(Token) would
// make the `insert_cons` logic simpler, since it could happen just there

pub fn tokenize(in_str: &str) -> Result<Vec<Token>> {
    let mut position = 0;
    let mut tokens = Vec::new();
    let mut remaining_chars = in_str.chars().peekable();

    while let Some(current_char) = remaining_chars.next() {
        // Returns at first error
        let token = munch_token(&mut remaining_chars, &current_char, position, &mut tokens)?;
        position = token.position.1; // new_position + current_char.len_utf8();
        tokens.push(token);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unclosed_character_class() {
        assert_eq!(
            tokenize(r"123[fdhk\]dfsdf"),
            Err(TokenizeError::UnterminatedCharacterSet(3))
        );
    }

    #[test]
    fn test_empty_positive_character_class() {
        assert_eq!(tokenize(r"abc[]"), Err(TokenizeError::EmptyCharacterSet(3)));
    }

    #[test]
    fn test_empty_negative_character_class() {
        assert_eq!(
            tokenize(r"abc[^]"),
            Err(TokenizeError::EmptyCharacterSet(3))
        );
    }

    #[test]
    fn test_unterminated_escape() {
        assert_eq!(tokenize(r"abc\"), Err(TokenizeError::UnterminatedEscape(3)));
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
            tokenize(r"abce[fg]+h*|i?j\kl[^a-c](abcd)i").unwrap()
        )
    }
}
