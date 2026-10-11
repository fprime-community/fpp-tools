//! Shortcuts that span lexer/parser abstraction.
//!
//! The way Rust works, parser doesn't necessary parse text, and you might
//! tokenize text without parsing it further. So, it makes sense to keep
//! abstract token parsing, and string tokenization as completely separate
//! layers.
//!
//! However, often you do parse text into syntax trees and the glue code for
//! that needs to live somewhere. Rather than putting it to lexer or parser, we
//! use a separate shortcuts module for that.

use std::mem;

use crate::{
    LexedStr, Step,
    SyntaxKind::{self, *},
};

#[derive(Debug)]
enum TriviaRegion {
    Keep(SyntaxKind),
    Replace(usize, SyntaxKind),
    RemoveAfter(usize, SyntaxKind),
    Remove(usize),
}

#[derive(Debug)]
pub enum StrStep<'a> {
    Token {
        kind: SyntaxKind,
        text: &'a str,
    },
    Enter {
        kind: SyntaxKind,
    },
    Exit,
    Error {
        msg: &'a str,
        pos: usize,
    },
    Expected {
        kind: SyntaxKind,
        start: usize,
        end: usize,
    },
}

impl LexedStr<'_> {
    /// Eat newlines, comments and whitespace given a starting token index.
    /// Return the token index of the next other token.
    ///
    /// This mirrors the Scala lexer's `eatNewlines`, which stops at an
    /// annotation.
    fn eat_newlines(&self, i: usize) -> usize {
        for ii in i..self.len() {
            match self.kind(ii) {
                EOL | COMMENT | WHITESPACE => (),
                _ => return ii,
            }
        }

        self.len()
    }

    /// Compute the trivia region at a point 'i': how the raw tokens starting
    /// there map to parser tokens.
    ///
    /// The rules are those of the Scala lexer:
    ///   * A newline or a comment, with the newlines, comments and whitespace
    ///     after it, is one end-of-line token, which is dropped before `)`,
    ///     `]` or `}`.
    ///   * An annotation and the symbols `*`, `/`, `-`, `->`, `+`, `=`, `;`,
    ///     `:`, `,`, `(`, `{` and `[` absorb the newlines after them.
    ///   * Whitespace and characters that cannot begin a token are skipped.
    fn as_trivia(&self, i: usize) -> TriviaRegion {
        assert!(i < self.len(), "{} < {}", i, self.len());

        let mut skip = 0;
        while skip + i < self.len() {
            match self.kind(i + skip) {
                // A character that cannot begin a token is reported by the
                // lexer and skipped like whitespace.
                WHITESPACE | UNKNOWN => {
                    skip += 1;
                }
                EOL | COMMENT => {
                    let after = self.eat_newlines(i + skip + 1);
                    let next = if after < self.len() {
                        self.kind(after)
                    } else {
                        EOF
                    };
                    return match next {
                        RIGHT_PAREN | RIGHT_CURLY | RIGHT_SQUARE => {
                            // Newlines before these closing tokens are dropped
                            TriviaRegion::Remove(after - i)
                        }
                        _ => {
                            // This is actually a newline delimiter
                            TriviaRegion::Replace(after - i, EOL)
                        }
                    };
                }

                // Tokens that eat newlines after them.
                k @ (PRE_ANNOTATION | POST_ANNOTATION | STAR | SLASH | MINUS | RIGHT_ARROW
                | PLUS | EQUALS | SEMI | COMMA | COLON | LEFT_PAREN | LEFT_CURLY
                | LEFT_SQUARE) => {
                    if skip > 0 {
                        return TriviaRegion::Remove(skip);
                    }

                    let after = self.eat_newlines(i + 1);
                    return TriviaRegion::RemoveAfter(after - i - 1, k);
                }

                k if skip == 0 => return TriviaRegion::Keep(k),
                _ => return TriviaRegion::Remove(skip),
            }
        }

        assert!(skip > 0);
        TriviaRegion::Remove(skip)
    }

    pub fn to_input(&self) -> crate::Input {
        let mut res = crate::Input::with_capacity(self.len());
        let mut i = 0;
        while i < self.len() {
            let t = self.as_trivia(i);
            match t {
                TriviaRegion::Keep(k) => {
                    // Normal token
                    res.push(k);
                    i += 1;
                }
                TriviaRegion::Replace(n, token) => {
                    res.push(token);
                    i += n;
                }
                TriviaRegion::RemoveAfter(n, token) => {
                    res.push(token);
                    i += n + 1;
                }
                TriviaRegion::Remove(n) => {
                    i += n;
                }
            }
        }

        res
    }

    /// NB: only valid to call with Output from Reparser/TopLevelEntry.
    pub fn intersperse_trivia(
        &self,
        output: &crate::Output,
        sink: &mut dyn FnMut(StrStep<'_>),
    ) -> bool {
        let mut builder = Builder {
            lexed: self,
            pos: 0,
            state: State::PendingEnter,
            sink,
        };

        for event in output.iter() {
            match event {
                Step::Token {
                    kind,
                    n_input_tokens: n_raw_tokens,
                } => builder.token(kind, n_raw_tokens),
                Step::Enter { kind } => builder.enter(kind),
                Step::Exit => builder.exit(),
                Step::Error { msg } => {
                    // An error is reported at the next token
                    builder.flush_pending_exit();
                    builder.eat_whitespace_outer();
                    let text_pos = builder.lexed.text_start(builder.pos);
                    (builder.sink)(StrStep::Error { msg, pos: text_pos });
                }
                Step::Expected { kind } => {
                    builder.flush_pending_exit();
                    let start = builder.lexed.text_start(builder.pos);
                    builder.eat_whitespace_outer();
                    let end = builder.lexed.text_start(builder.pos);
                    (builder.sink)(StrStep::Expected { kind, start, end });
                }
            }
        }

        match mem::replace(&mut builder.state, State::Normal) {
            State::PendingExit => {
                builder.eat_trivias();
                (builder.sink)(StrStep::Exit);
            }
            State::PendingEnter | State::Normal => unreachable!(),
        }

        // is_eof?
        builder.pos == builder.lexed.len()
    }
}

struct Builder<'a, 'b> {
    lexed: &'a LexedStr<'a>,
    pos: usize,
    state: State,
    sink: &'b mut dyn FnMut(StrStep<'_>),
}

enum State {
    PendingEnter,
    Normal,
    PendingExit,
}

impl Builder<'_, '_> {
    fn token(&mut self, kind: SyntaxKind, n_tokens: u8) {
        match mem::replace(&mut self.state, State::Normal) {
            State::PendingEnter => unreachable!(),
            State::PendingExit => (self.sink)(StrStep::Exit),
            State::Normal => (),
        }

        let t = self.lexed.as_trivia(self.pos);

        match t {
            TriviaRegion::Keep(t_kind) => {
                assert_eq!(kind, t_kind);
                self.do_token(kind, n_tokens as usize);
            }
            TriviaRegion::Replace(n, t_kind) => {
                assert_eq!(kind, t_kind);
                assert_eq!(n_tokens, 1);
                for _ in 0..n {
                    let kind = self.lexed.kind(self.pos);
                    self.do_token(kind, 1);
                }
            }
            TriviaRegion::RemoveAfter(n, t_kind) => {
                assert_eq!(kind, t_kind);
                assert_eq!(n_tokens, 1);
                self.do_token(kind, 1);

                for _ in 0..n {
                    let kind = self.lexed.kind(self.pos);
                    self.do_token(kind, 1);
                }
            }
            TriviaRegion::Remove(n) => {
                for _ in 0..n {
                    self.do_token(self.lexed.kind(self.pos), 1);
                }

                self.token(kind, n_tokens);
            }
        }
    }

    fn enter(&mut self, kind: SyntaxKind) {
        match mem::replace(&mut self.state, State::Normal) {
            State::PendingEnter => {
                (self.sink)(StrStep::Enter { kind });
                // No need to attach trivias to previous node: there is no
                // previous node.
                return;
            }
            State::PendingExit => (self.sink)(StrStep::Exit),
            State::Normal => (),
        }

        self.eat_whitespace_outer();
        (self.sink)(StrStep::Enter { kind });
    }

    /// Finish a node whose exit is pending, so that what follows lies
    /// outside it.
    fn flush_pending_exit(&mut self) {
        if let State::PendingExit = self.state {
            self.state = State::Normal;
            (self.sink)(StrStep::Exit);
        }
    }

    fn exit(&mut self) {
        match mem::replace(&mut self.state, State::PendingExit) {
            State::PendingEnter => unreachable!(),
            State::PendingExit => (self.sink)(StrStep::Exit),
            State::Normal => (),
        }
    }

    fn eat_trivias(&mut self) {
        if self.pos >= self.lexed.len() {
            return;
        }

        if let TriviaRegion::Remove(n) = self.lexed.as_trivia(self.pos) {
            for _ in 0..n {
                let kind = self.lexed.kind(self.pos);
                self.do_token(kind, 1);
            }
        }
    }

    fn eat_whitespace_outer(&mut self) {
        if self.pos >= self.lexed.len() {
            return;
        }

        if let TriviaRegion::Remove(n) = self.lexed.as_trivia(self.pos) {
            for _ in 0..n {
                let kind = self.lexed.kind(self.pos);
                match kind {
                    WHITESPACE | COMMENT | UNKNOWN => {
                        self.do_token(kind, 1);
                    }
                    _ => break,
                }
            }
        }
    }

    fn do_token(&mut self, kind: SyntaxKind, n_tokens: usize) {
        let text = &self.lexed.range_text(self.pos..self.pos + n_tokens);
        self.pos += n_tokens;
        (self.sink)(StrStep::Token { kind, text });
    }
}
