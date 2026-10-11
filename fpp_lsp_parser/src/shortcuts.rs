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
    /// Eat newlines comments and whitespace given a starting token index
    /// Return the token index of the next non-whitespace token
    fn eat_newlines(&self, i: usize) -> usize {
        for ii in i..self.len() {
            match self.kind(ii) {
                EOL | COMMENT | WHITESPACE | PRE_ANNOTATION | POST_ANNOTATION => (),
                _ => return ii,
            }
        }

        self.len()
    }

    /// Like eat_newlines but stops at annotations instead of consuming them
    fn eat_newlines_without_annotations(&self, i: usize) -> usize {
        for ii in i..self.len() {
            match self.kind(ii) {
                EOL | COMMENT | WHITESPACE => (),
                _ => return ii, // Stop at annotations or any other token
            }
        }

        self.len()
    }

    /// Compute the number of trivias at a point 'i'.
    /// Some trivias may be reduced into a 'psuedo' token, return the psuedo token
    /// as the second option in the tuple.
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
                EOL | COMMENT | PRE_ANNOTATION | POST_ANNOTATION => {
                    let after = self.eat_newlines(i + skip + 1);
                    return match self.kind(i) {
                        RIGHT_PAREN | RIGHT_CURLY | RIGHT_SQUARE => {
                            // Whitespace behind these closing tokens are dropped
                            TriviaRegion::Remove(after - i)
                        }
                        _ => {
                            // This is actually a newline delimiter
                            TriviaRegion::Replace(after - i, EOL)
                        }
                    };
                }

                // Tokens that eat newlines after them.
                STAR | SLASH | MINUS | PLUS | EQUALS | SEMI | COMMA | COLON | LEFT_PAREN
                | LEFT_CURLY | LEFT_SQUARE => {
                    if skip > 0 {
                        return TriviaRegion::Remove(skip);
                    }

                    let after = self.eat_newlines(i + 1);
                    // For LEFT_CURLY, don't include annotations in RemoveAfter
                    // They should be attached to the definitions inside, not to the brace
                    let after = if self.kind(i) == LEFT_CURLY {
                        self.eat_newlines_without_annotations(i + 1)
                    } else {
                        after
                    };
                    return TriviaRegion::RemoveAfter(after - i - 1, self.kind(i));
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
                    let text_pos = builder.lexed.text_start(builder.pos);
                    (builder.sink)(StrStep::Error { msg, pos: text_pos });
                }
                Step::Expected { kind } => {
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

        // Don't emit annotations when entering member list nodes
        // They should be attached to the individual member definitions instead
        let emit_annotations = !Self::is_member_list(kind);
        self.eat_whitespace_inner(emit_annotations);
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

    fn eat_whitespace_inner(&mut self, emit_annotations: bool) {
        if self.pos >= self.lexed.len() {
            return;
        }

        if let TriviaRegion::Remove(n) = self.lexed.as_trivia(self.pos) {
            for _ in 0..n {
                let kind = self.lexed.kind(self.pos);
                match kind {
                    WHITESPACE => {
                        self.do_token(kind, 1);
                    }
                    PRE_ANNOTATION | POST_ANNOTATION => {
                        if emit_annotations {
                            self.do_token(kind, 1);
                        } else {
                            // Don't emit, will be attached to next definition
                            break;
                        }
                    }
                    _ => break,
                }
            }
        }
    }

    fn is_member_list(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            MODULE_MEMBER_LIST
                | COMPONENT_MEMBER_LIST
                | INTERFACE_MEMBER_LIST
                | STRUCT_MEMBER_LIST
                | ENUM_MEMBER_LIST
                | STATE_MACHINE_MEMBER_LIST
                | STATE_MEMBER_LIST
                | TOPOLOGY_MEMBER_LIST
                | TLM_PACKET_SET_MEMBER_LIST
                | TLM_PACKET_MEMBER_LIST
        )
    }

    fn do_token(&mut self, kind: SyntaxKind, n_tokens: usize) {
        let text = &self.lexed.range_text(self.pos..self.pos + n_tokens);
        self.pos += n_tokens;
        (self.sink)(StrStep::Token { kind, text });
    }
}
