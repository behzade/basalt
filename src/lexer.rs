// lexer.rs

// Import the Token enum and the identifier lookup function from the token module.
// This assumes `token.rs` is in the same directory or a parent module.
use crate::token::{lookup_ident, Token};

/// The Lexer struct holds the state required for tokenizing source code.
pub struct Lexer {
    input: Vec<char>,      // The source code as a vector of characters for easy indexing.
    position: usize,       // The current position in `input` (points to the current character).
    read_position: usize,  // The next reading position in `input` (one character ahead of `position`).
    ch: char,              // The current character being examined.
}

impl Lexer {
    /// Creates a new Lexer instance.
    ///
    /// # Arguments
    ///
    /// * `input` - A String containing the source code to be tokenized.
    pub fn new(input: String) -> Self {
        let mut lexer = Lexer {
            input: input.chars().collect(),
            position: 0,
            read_position: 0,
            ch: '\0', // Use NUL character as a sentinel for "not read yet" or EOF.
        };
        lexer.read_char(); // Initialize the first character.
        lexer
    }

    /// Reads the next character from the input and advances the lexer's position.
    fn read_char(&mut self) {
        if self.read_position >= self.input.len() {
            self.ch = '\0'; // End of input.
        } else {
            self.ch = self.input[self.read_position];
        }
        self.position = self.read_position;
        self.read_position += 1;
    }

    /// Peeks at the next character in the input without consuming it.
    fn peek_char(&self) -> char {
        if self.read_position >= self.input.len() {
            '\0'
        } else {
            self.input[self.read_position]
        }
    }

    /// Skips over any whitespace characters (spaces, tabs, newlines).
    fn skip_whitespace(&mut self) {
        while self.ch.is_whitespace() {
            self.read_char();
        }
    }
    
    /// Skips over a single-line comment (from `//` to the end of the line).
    fn skip_comment(&mut self) {
        while self.ch != '\n' && self.ch != '\0' {
            self.read_char();
        }
    }

    /// Reads a complete identifier or keyword from the input.
    fn read_identifier(&mut self) -> String {
        let start_pos = self.position;
        // An identifier starts with a letter or `_`, and can be followed by letters, numbers, or `_`.
        while self.is_letter() || self.ch.is_ascii_digit() {
            self.read_char();
        }
        self.input[start_pos..self.position].iter().collect()
    }

    /// Reads a number literal (integer or float) from the input.
    fn read_number(&mut self) -> Token {
        let start_pos = self.position;
        let mut has_dot = false;
        while self.ch.is_ascii_digit() || self.ch == '.' {
            if self.ch == '.' {
                if has_dot { break; } // Can't have more than one decimal point.
                has_dot = true;
            }
            self.read_char();
        }
        let literal: String = self.input[start_pos..self.position].iter().collect();

        if has_dot {
            match literal.parse::<f64>() {
                Ok(f) => Token::Float(f),
                Err(_) => Token::Illegal, // Failed to parse as a float.
            }
        } else {
            match literal.parse::<i64>() {
                Ok(i) => Token::Int(i),
                Err(_) => Token::Illegal, // Failed to parse as an integer.
            }
        }
    }

    /// Reads a string literal enclosed in double quotes.
    fn read_string(&mut self) -> Token {
        let start_pos = self.position + 1; // Skip the opening quote.
        loop {
            self.read_char();
            // Note: This simple version doesn't handle escape sequences like `\"`.
            if self.ch == '"' || self.ch == '\0' {
                break;
            }
        }
        let literal: String = self.input[start_pos..self.position].iter().collect();
        Token::String(literal)
    }

    /// Helper to check if the current character is a letter or underscore.
    fn is_letter(&self) -> bool {
        self.ch.is_alphabetic() || self.ch == '_'
    }

    /// The main method to get the next token from the source code.
    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let token = match self.ch {
            // --- Two-character operators ---
            '=' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::Eq
                } else {
                    Token::Assign
                }
            }
            '!' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::NotEq
                } else {
                    Token::Not
                }
            }
            '<' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::LtEq
                } else {
                    Token::Lt
                }
            }
            '>' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::GtEq
                } else {
                    Token::Gt
                }
            }
            '-' => {
                if self.peek_char() == '>' {
                    self.read_char();
                    Token::Arrow
                } else {
                    Token::Minus
                }
            }
            ':' => {
                if self.peek_char() == ':' {
                    self.read_char();
                    Token::DoubleColon
                } else {
                    Token::Colon
                }
            }
            '*' => {
                if self.peek_char() == '*' {
                    self.read_char();
                    Token::Power
                } else {
                    Token::Star
                }
            }
            '&' => {
                if self.peek_char() == '&' {
                    self.read_char();
                    Token::And
                } else {
                    Token::Illegal // Single '&' is not a valid token.
                }
            }
            '|' => {
                if self.peek_char() == '|' {
                    self.read_char();
                    Token::Or
                } else {
                    Token::Illegal // Single '|' is not a valid token.
                }
            }
            '/' => {
                if self.peek_char() == '/' {
                    self.skip_comment();
                    return self.next_token(); // Get the token after the comment.
                } else {
                    Token::Slash
                }
            }
            
            // --- Single-character tokens ---
            '+' => Token::Plus,
            '%' => Token::Percent,
            ',' => Token::Comma,
            ';' => Token::Semicolon,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            '.' => Token::Dot,

            // --- Literals ---
            '"' => {
                let tok = self.read_string();
                // read_string consumes the closing quote, so we advance once more to move past it.
                self.read_char(); 
                return tok;
            }

            // --- End of File ---
            '\0' => Token::Eof,

            // --- Identifiers, Keywords, and Numbers ---
            _ => {
                if self.is_letter() {
                    let ident = self.read_identifier();
                    // read_identifier advances the cursor past the identifier, so we return directly.
                    return lookup_ident(&ident);
                } else if self.ch.is_ascii_digit() {
                    // read_number also advances the cursor.
                    return self.read_number();
                } else {
                    // Unrecognized character.
                    Token::Illegal
                }
            }
        };

        self.read_char(); // Advance the lexer for the next token.
        token
    }
}

// --- Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::Token;

    #[test]
    fn test_next_token() {
        let input = r#"
            let five = 5;
            let ten = 10;
            let pi = 3.14;

            let add = fn(x, y) {
                x + y;
            };

            let result = add(five, ten);
            
            // Operators
            !-/*5;
            5 < 10 > 5;
            10 == 10;
            10 != 9;
            1 <= 2;
            2 >= 1;
            2 ** 3;
            true && false;
            true || false;

            if (5 < 10) {
                return true;
            } else {
                return false;
            }

            let s = "hello world";
            let arr = [1, 2];
            let point = myMod::Point { x: 1, y: 2 };
        "#;

        let tests = vec![
            Token::Let, Token::Ident("five".to_string()), Token::Assign, Token::Int(5), Token::Semicolon,
            Token::Let, Token::Ident("ten".to_string()), Token::Assign, Token::Int(10), Token::Semicolon,
            Token::Let, Token::Ident("pi".to_string()), Token::Assign, Token::Float(3.14), Token::Semicolon,
            Token::Let, Token::Ident("add".to_string()), Token::Assign, Token::Fn, Token::LParen, Token::Ident("x".to_string()), Token::Comma, Token::Ident("y".to_string()), Token::RParen, Token::LBrace,
            Token::Ident("x".to_string()), Token::Plus, Token::Ident("y".to_string()), Token::Semicolon,
            Token::RBrace, Token::Semicolon,
            Token::Let, Token::Ident("result".to_string()), Token::Assign, Token::Ident("add".to_string()), Token::LParen, Token::Ident("five".to_string()), Token::Comma, Token::Ident("ten".to_string()), Token::RParen, Token::Semicolon,
            Token::Not, Token::Minus, Token::Slash, Token::Star, Token::Int(5), Token::Semicolon,
            Token::Int(5), Token::Lt, Token::Int(10), Token::Gt, Token::Int(5), Token::Semicolon,

            Token::Int(10), Token::Eq, Token::Int(10), Token::Semicolon,
            Token::Int(10), Token::NotEq, Token::Int(9), Token::Semicolon,
            Token::Int(1), Token::LtEq, Token::Int(2), Token::Semicolon,
            Token::Int(2), Token::GtEq, Token::Int(1), Token::Semicolon,
            Token::Int(2), Token::Power, Token::Int(3), Token::Semicolon,
            Token::Bool(true), Token::And, Token::Bool(false), Token::Semicolon,
            Token::Bool(true), Token::Or, Token::Bool(false), Token::Semicolon,

            Token::If, Token::LParen, Token::Int(5), Token::Lt, Token::Int(10), Token::RParen, Token::LBrace,
            Token::Return, Token::Bool(true), Token::Semicolon,
            Token::RBrace, Token::Else, Token::LBrace,
            Token::Return, Token::Bool(false), Token::Semicolon,
            Token::RBrace,

            Token::Let, Token::Ident("s".to_string()), Token::Assign, Token::String("hello world".to_string()), Token::Semicolon,
            Token::Let, Token::Ident("arr".to_string()), Token::Assign, Token::LBracket, Token::Int(1), Token::Comma, Token::Int(2), Token::RBracket, Token::Semicolon,
            Token::Let, Token::Ident("point".to_string()), Token::Assign, Token::Ident("myMod".to_string()), Token::DoubleColon, Token::Ident("Point".to_string()), Token::LBrace, Token::Ident("x".to_string()), Token::Colon, Token::Int(1), Token::Comma, Token::Ident("y".to_string()), Token::Colon, Token::Int(2), Token::RBrace, Token::Semicolon,
            Token::Eof,
        ];

        let mut lexer = Lexer::new(input.to_string());

        for (i, expected_token) in tests.iter().enumerate() {
            let token = lexer.next_token();
            println!("Test {}: Expected {:?}, Got {:?}", i, expected_token, token);
            assert_eq!(*expected_token, token);
        }
    }
}

