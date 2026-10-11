//! Port of the Scala lexer unit tests (`LexerSpec` in
//! `compiler/lib/src/test/scala/syntax/Lexer.scala`).
//!
//! The Scala scanner both splits the input into tokens and computes their
//! values, so its Rust counterpart is the cursor over the `fpp_lexer` tokens.
//! The input files are in `tests/lexer/{ok,error}`.

use crate::cursor::Cursor;
use fpp_core::SourceFile;
use fpp_lexer::TokenKind;
use std::path::PathBuf;

/// Scan `s` into its tokens as kinds and texts, or return the lexer errors
fn list(s: &str) -> Result<Vec<(TokenKind, String)>, String> {
    let mut diagnostics = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
    let tokens = fpp_core::run(&mut ctx, || {
        let file = SourceFile::new("<stdin>", s.to_string());
        let mut cursor = Cursor::new(file, s, None);
        let mut tokens = vec![];
        while let Some(token) = cursor.next() {
            tokens.push((token.kind(), token.str().to_string()));
        }

        tokens
    });
    drop(ctx);

    if diagnostics.is_empty() {
        Ok(tokens)
    } else {
        Err(String::from_utf8_lossy(&diagnostics).to_string())
    }
}

/// The `.fpp` files in `tests/lexer/<dir>`
fn input_files(dir: &str) -> Vec<PathBuf> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/lexer");
    path.push(dir);

    let mut files: Vec<_> = std::fs::read_dir(&path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "fpp"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no input files in {}", path.display());
    files
}

#[test]
fn error_not_lex() {
    for file in input_files("error") {
        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(list(&contents).is_err(), "lexed {}", file.display());
    }
}

#[test]
fn fp_literal() {
    let literals = ["123.456", "123.", "456.", "123e10", "123e-10"];
    for s in literals {
        assert_eq!(
            list(s),
            Ok(vec![(TokenKind::LiteralFloat, s.to_string())]),
            "lex {s} to an FP literal"
        );
    }
}

#[test]
fn ok_lex() {
    for file in input_files("ok") {
        let contents = std::fs::read_to_string(&file).unwrap();
        if let Err(err) = list(&contents) {
            panic!("could not lex {}: {err}", file.display());
        }
    }
}

#[test]
fn annotations() {
    assert_eq!(
        list("@ abc"),
        Ok(vec![(TokenKind::PreAnnotation, "abc".to_string())]),
        "lex \"@ abc\" to a pre-annotation"
    );
    assert_eq!(
        list("@< abc"),
        Ok(vec![(TokenKind::PostAnnotation, "abc".to_string())]),
        "lex \"@< abc\" to a post-annotation"
    );
}

#[test]
fn int_literal() {
    let literals = ["123456", "0x123456ABCD"];
    for s in literals {
        assert_eq!(
            list(s),
            Ok(vec![(TokenKind::LiteralInt, s.to_string())]),
            "lex {s} to an int literal"
        );
    }
}

#[test]
fn line_continuation() {
    assert_eq!(
        list("\n"),
        Ok(vec![(TokenKind::Eol, String::new())]),
        "lex \"\\n\" to one EOL"
    );
    assert_eq!(list("\\\n"), Ok(vec![]), "lex \"\\\\n\" to zero tokens");
}

#[test]
fn string_literal() {
    let literals = [
        "\"abc\"",
        "\"\\\" abc\"",
        "\"abc \\\"\"",
        "\"abc \\\" abc\"",
        "\"\"\"abc\"\"\"",
        "\"\"\"\\\" abc\"\"\"",
        "\"\"\"abc \\\"\"\"\"",
        "\"\"\"abc \\\" abc\"\"\"",
        "\"\"\"abc \" abc\"\"\"",
        "\"\"\"abc \"\" abc\"\"\"",
        "\"\\\\q\"",
        "\"\"\"\\\\q\"\"\"",
    ];
    for s in literals {
        let tokens = list(s).unwrap_or_else(|err| panic!("could not lex {s}: {err}"));
        assert!(
            matches!(
                tokens.as_slice(),
                [(
                    TokenKind::LiteralString | TokenKind::LiteralMultilineString { .. },
                    _
                )]
            ),
            "lex {s} to a string literal: {tokens:?}"
        );
    }
}
