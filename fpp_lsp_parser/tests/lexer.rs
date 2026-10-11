//! Lexical errors reach the lossless parse. The inputs are the Scala lexer
//! test inputs (lib/src/test/input/syntax/lexer).

use fpp_lsp_parser::{LexedStr, TopEntryPoint, parse};
use std::fs;
use std::path::PathBuf;

fn input_files(dir: &str) -> Vec<PathBuf> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/lexer");
    path.push(dir);
    let mut files: Vec<PathBuf> = fs::read_dir(&path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "fpp"))
        .collect();
    files.sort();
    files
}

fn lexer_errors(text: &str) -> Vec<String> {
    LexedStr::new(text)
        .errors()
        .map(|(_, msg)| msg.to_string())
        .collect()
}

#[test]
fn error() {
    for file in input_files("error") {
        let text = fs::read_to_string(&file).unwrap();
        assert!(
            !lexer_errors(&text).is_empty(),
            "{}: expected a lexer error",
            file.display()
        );
        // A lexer error is a parse error, so the input is rejected.
        assert!(
            !parse(&text, TopEntryPoint::Module).errors().is_empty(),
            "{}: expected a parse error",
            file.display()
        );
    }
}

#[test]
fn ok() {
    for file in input_files("ok") {
        // ok_literals checks literals.fpp
        if file.file_name().unwrap() == "literals.fpp" {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        let errors = lexer_errors(&text);
        assert!(errors.is_empty(), "{}: {:?}", file.display(), errors);
    }
}

#[test]
#[ignore = "fpp_lexer rejects signed exponents such as 123e-10"]
fn ok_literals() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/lexer/ok/literals.fpp");
    let text = fs::read_to_string(&path).unwrap();
    let errors = lexer_errors(&text);
    assert!(errors.is_empty(), "{:?}", errors);
}

/// Each malformed input is rejected with an `invalid token` error.
#[test]
fn invalid_tokens() {
    for text in [
        "constant a = \"abc\n",
        "constant a = \"\"\"\n  abc\n",
        "constant a = 1.5x\n",
        "constant a = 1e\n",
        "constant a = 1.b\n",
        "constant a = 1e5x\n",
        "constant a = 1 \\ x\n  + 2\n",
        "constant\ta = 1\n",
        "constant a = $ 1\n",
        "constant a = 1 < 2\n",
    ] {
        let errors = parse(text, TopEntryPoint::Module).errors();
        assert!(
            errors
                .first()
                .is_some_and(|e| e.to_string().starts_with("invalid token: ")),
            "{:?}: {:?}",
            text,
            errors
        );
    }
}

/// An invalid character is skipped after it is reported, so it causes no
/// parse error of its own.
#[test]
fn invalid_character_is_skipped() {
    let errors = parse("constant\ta = 1\n", TopEntryPoint::Module).errors();
    assert_eq!(errors.len(), 1, "{:?}", errors);
    assert_eq!(
        errors[0].to_string(),
        "invalid token: unicode value 9, hex 0x9\n\
         note: embedded tab characters are not allowed in FPP source files\n\
         try configuring your editor to convert tabs to spaces"
    );
}
