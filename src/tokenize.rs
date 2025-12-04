// todo: tokenizer
// input pattern -> tokens -> parse? into NFA
// TODO: visualize!

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Token {
    Cons,                    // Concatenation
    OpenGroup,               // (
    CloseGroup,              // (
    LeftSquareBracket,       // [
    RightSquareBracket,      // ]
    Or,                      // |
    ZeroOrMore,              // *
    OneOrMore,               // +
    ZeroOrOne,               // ?
    Backslash(usize),        // \
    Character(usize),        // ., a, b, !, etc.
    EscapedCharacter(usize), // \t, \n, \\, etc.
    CharacterSet(usize, usize, bool), // [a-c], etc; [^a-c], etc when bool is false
                             // Whitespace(usize),
                             // NonWhitespace(usize),
                             // Digit(usize),
                             // NonDigit(usize),
                             // Word(usize),
                             // NonWord(usize),
}

fn should_join_literals(token: &Token) -> bool {
    match &token {
        Token::CloseGroup
        | Token::ZeroOrMore
        | Token::OneOrMore
        | Token::ZeroOrOne
        | Token::Character(_)
        | Token::EscapedCharacter(_)
        | Token::CharacterSet(_, _, _) => true,
        _ => false,
    }
}

pub enum Arity {
    Binary,
    Unary,
    NoOp,
}

impl Token {
    fn get_precedence(self: &Self) -> i8 {
        -(match &self {
            Token::LeftSquareBracket | Token::RightSquareBracket => 3,
            // TODO: OpenGroup might not need to be here; aka should not get popped from op stack
            // if lower priority operator is encountered
            Token::CloseGroup => 4,
            Token::ZeroOrMore | Token::OneOrMore | Token::ZeroOrOne => 5,
            Token::Cons => 6,
            Token::Or => 8,
            Token::OpenGroup => 10, // CloseGroup represents () grouping operator precedence, this
            // needs to be low so it doesn't mess with ordering with other
            // operators
            _ => 0,
        })
    }

    pub fn precedes(self: &Self, other: &Self) -> bool {
        self.get_precedence() > other.get_precedence()
    }

    pub fn same_precedence_as(self: &Self, other: &Self) -> bool {
        self.get_precedence() == other.get_precedence()
    }

    pub fn arity(self: &Self) -> Arity {
        match self {
            Token::ZeroOrMore | Token::OneOrMore | Token::ZeroOrOne | Token::CloseGroup => {
                Arity::Unary
            }
            Token::Cons | Token::Or => Arity::Binary,
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

fn re_internal_munch_character_class(input: &[u8], position: usize) -> (Token, usize) {
    let remaining_input = &input[position + 1..];
    let mut found: Option<usize> = None;
    // TODO: test case for failure where there is not another character

    for index in 0..remaining_input.len() {
        println!("pos {:?}", &remaining_input[index..index + 1]);
        if &remaining_input[index..index + 1] == b"]" {
            found = Some(position + index + 1);
            break;
        }
    }

    if let Some(close_bracket_position) = found {
        (
            Token::CharacterSet(
                position,
                close_bracket_position + 1, // need the position AFTER the ]
                &remaining_input[0..1] == b"^",
            ),
            close_bracket_position,
        )
    } else {
        panic!("Unterminated character set at {}", position);
    }
}

// TODO: improve name; it inserts a cons if necessary
fn insert_cons(tokens: &mut Vec<Token>) {
    if let Some(token) = tokens.last() {
        if should_join_literals(token) {
            tokens.push(Token::Cons);
        }
    }
}

fn re_internal_munch_token(
    input: &[u8],
    character: &[u8],
    position: usize,
    tokens: &mut Vec<Token>,
) -> (Token, usize) {
    let mut new_position = position;
    (
        match character {
            b"(" => Token::OpenGroup,
            b")" => Token::CloseGroup,
            b"[" => {
                let (token, end_char_class) = re_internal_munch_character_class(input, position);
                new_position = end_char_class;
                println!("character set end {:?}", new_position);
                println!("token: {:?}", token);
                insert_cons(tokens);
                token
            }
            b"|" => Token::Or,
            b"*" => Token::ZeroOrMore,
            b"+" => Token::OneOrMore,
            b"?" => Token::ZeroOrOne,
            b"\\" => {
                new_position += 1;
                insert_cons(tokens);
                Token::EscapedCharacter(position)
            }
            _ => {
                insert_cons(tokens);
                Token::Character(position)
            }
        },
        new_position,
    )
}

pub fn re_tokenize(input: &[u8]) -> (Vec<Token>, &[u8]) {
    let mut position = 0;
    let mut tokens = Vec::new();
    let num_chars = input.len();
    let max_position = num_chars - 1;
    println!("max: {:?}", max_position);

    while position < num_chars {
        println!("{:?}", position);
        let (token, new_position) =
            re_internal_munch_token(input, &input[position..position + 1], position, &mut tokens);
        position = new_position + 1;
        tokens.push(token);
    }

    (tokens, input)
}

// fn re_example_pattern<'a>() {
//     // ab|c*d
//     let tokens = [
//         Token {
//             token_type: TokenType::Character,
//             start: 0,
//             end: 1,
//         },
//         Token {
//             token_type: TokenType::Character,
//             start: 1,
//             end: 2,
//         },
//         Token {
//             token_type: TokenType::OR,
//             start: 2,
//             end: 3,
//         },
//         Token {
//             token_type: TokenType::Character,
//             start: 3,
//             end: 4,
//         },
//         Token {
//             token_type: TokenType::ZeroOrMore,
//             start: 4,
//             end: 5,
//         },
//         Token {
//             token_type: TokenType::Character,
//             start: 5,
//             end: 6,
//         },
//     ];
//     // ab+c*d?
//     let pattern = Pattern {
//         transitions: vec![
//             HashMap::from([("a", vec![1])]),
//             HashMap::from([("b", vec![2])]),
//             HashMap::from([("b", vec![2]), (UNIT, vec![3])]),
//             HashMap::from([("c", vec![3]), (UNIT, vec![4]), ("d", vec![4])]),
//         ],
//         start_state: 0,
//         accept_states: HashSet::from([4]),
//     };

//     // abc+|ab*d?
//     let pattern = Pattern {
//         transitions: vec![
//             HashMap::from([("a", vec![1, 4])]),
//             HashMap::from([("b", vec![2])]),
//             HashMap::from([("b", vec![2]), ("c", vec![3])]),
//             HashMap::from([]), // TODO: consider Option, since that might prevent memory alloc
//             HashMap::from([(UNIT, vec![5])]),
//             HashMap::from([(UNIT, vec![6]), ("d", vec![6]), ("b", vec![5])]),
//         ],
//         start_state: 0,
//         accept_states: HashSet::from([3, 6]),
//     };

//     let token_string = ["a", "b", "c+", "|", "a", "b*", "d?"];
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize() {
        // pattern: `ab+`
        assert_eq!(
            vec![
                Token::Character(0),
                Token::Cons,
                Token::Character(1),
                Token::Cons,
                Token::Character(2),
                Token::Cons,
                Token::Character(3),
                Token::Cons,
                Token::CharacterSet(4, 8, false),
                Token::OneOrMore,
                Token::Cons,
                Token::Character(9),
                Token::ZeroOrMore,
                Token::Or,
                Token::Character(12),
                Token::ZeroOrOne,
                Token::Cons,
                Token::Character(14),
                Token::Cons,
                Token::EscapedCharacter(15),
                Token::Cons,
                Token::Character(17),
                Token::Cons,
                Token::CharacterSet(18, 24, true),
                Token::OpenGroup,
                Token::Character(25),
                Token::Cons,
                Token::Character(26),
                Token::Cons,
                Token::Character(27),
                Token::Cons,
                Token::Character(28),
                Token::CloseGroup,
                Token::Cons,
                Token::Character(30)
            ],
            re_tokenize(b"abce[fg]+h*|i?j\\kl[^a-c](abcd)i").0
        )
    }
}
