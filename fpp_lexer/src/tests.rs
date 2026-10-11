use crate::TokenKind::*;
use crate::{KeywordKind, TokenKind};

use pretty_assertions::assert_eq;

#[derive(Debug, PartialEq)]
struct TokenStr<'a>(pub &'a str, pub TokenKind);

/// A lexer error as the text it covers and its message
type ErrorStr = (String, String);

/// Lex `content` into its tokens and errors
fn lex_with_errors(content: &'_ str) -> (Vec<TokenStr<'_>>, Vec<ErrorStr>) {
    let mut out = Vec::new();
    let mut offset: usize = 0;
    let mut lexer = crate::Lexer::new(content);
    for token in lexer.by_ref() {
        out.push(TokenStr(&content[offset..(offset + token.len)], token.kind));
        offset += token.len;
    }

    let errors = lexer
        .errors()
        .map(|err| {
            (
                content[err.pos..err.pos + err.len].to_string(),
                err.msg.clone(),
            )
        })
        .collect();
    (out, errors)
}

/// Lex `content`, which must lex without errors
fn lex(content: &'_ str) -> Vec<TokenStr<'_>> {
    let (tokens, errors) = lex_with_errors(content);
    assert_eq!(errors, vec![], "unexpected lexer errors");
    tokens
}

fn errors(content: &str) -> Vec<ErrorStr> {
    lex_with_errors(content).1
}

fn expected_errors(errors: &[(&str, &str)]) -> Vec<ErrorStr> {
    errors
        .iter()
        .map(|(text, msg)| (text.to_string(), msg.to_string()))
        .collect()
}

#[test]
fn whitespace() {
    let tokens = lex("   ");
    assert_eq!(tokens, vec![TokenStr("   ", Whitespace)]);
}

#[test]
fn comment() {
    let tokens = lex(r#" # comment

    # more comments

    "#);
    assert_eq!(
        tokens,
        vec![
            TokenStr(" ", Whitespace),
            TokenStr("# comment", Comment),
            TokenStr("\n\n", Eol),
            TokenStr("    ", Whitespace),
            TokenStr("# more comments", Comment),
            TokenStr("\n\n", Eol),
            TokenStr("    ", Whitespace),
        ]
    );
}

#[test]
fn literals() {
    let tokens = lex(
        r#"12 1.23 0x10 0x1AEF 001 1e30 10.3e3 .3e3 "" "string \"" """
    a multiline literal string with \"\"\" some \escapes " "
    """ """""" "#,
    );
    assert_eq!(
        tokens,
        vec![
            TokenStr("12", LiteralInt),
            TokenStr(" ", Whitespace),
            TokenStr("1.23", LiteralFloat),
            TokenStr(" ", Whitespace),
            TokenStr("0x10", LiteralInt),
            TokenStr(" ", Whitespace),
            TokenStr("0x1AEF", LiteralInt),
            TokenStr(" ", Whitespace),
            TokenStr("001", LiteralInt),
            TokenStr(" ", Whitespace),
            TokenStr("1e30", LiteralFloat),
            TokenStr(" ", Whitespace),
            TokenStr("10.3e3", LiteralFloat),
            TokenStr(" ", Whitespace),
            TokenStr(".3e3", LiteralFloat),
            TokenStr(" ", Whitespace),
            TokenStr("\"\"", LiteralString),
            TokenStr(" ", Whitespace),
            TokenStr("\"string \\\"\"", LiteralString),
            TokenStr(" ", Whitespace),
            TokenStr(
                "\"\"\"\n    a multiline literal string with \\\"\\\"\\\" some \\escapes \" \"\n    \"\"\"",
                LiteralMultilineString { indent: 4 }
            ),
            TokenStr(" ", Whitespace),
            TokenStr("\"\"\"\"\"\"", LiteralMultilineString { indent: 0 }),
            TokenStr(" ", Whitespace),
        ]
    );
}

#[test]
fn float_literals() {
    for s in [
        "123.456", "123.", "456.", "123e10", "123e-10", "1.5E+3", "1E+5", ".5e-2", "1.e5",
    ] {
        assert_eq!(lex(s), vec![TokenStr(s, LiteralFloat)], "{s}");
    }
}

#[test]
fn int_literals() {
    for s in ["123456", "0x123456ABCD", "0X1f", "0", "007"] {
        assert_eq!(lex(s), vec![TokenStr(s, LiteralInt)], "{s}");
    }
}

#[test]
fn exponent_without_digits() {
    // The exponent letter is not part of the literal
    assert_eq!(
        lex_with_errors("1e-x").0,
        vec![
            TokenStr("1", LiteralFloat),
            TokenStr("e", Identifier),
            TokenStr("-", Minus),
            TokenStr("x", Identifier),
        ]
    );
    assert_eq!(
        errors("1e-x"),
        expected_errors(&[("1e", "invalid literal number")])
    );
}

#[test]
fn letter_after_int_literal() {
    // A digit outside the base ends the literal
    for (s, int, rest) in [
        ("15a", "15", "a"),
        ("9A", "9", "A"),
        ("0x1g", "0x1", "g"),
        ("0x1G", "0x1", "G"),
    ] {
        assert_eq!(
            lex(s),
            vec![TokenStr(int, LiteralInt), TokenStr(rest, Identifier)],
            "{s}"
        );
    }
}

#[test]
fn hex_prefix_without_digits() {
    for s in ["0x", "0X", "0xg", "0xG1"] {
        assert_eq!(
            errors(s),
            expected_errors(&[(&s[..2], "invalid literal number")]),
            "{s}"
        );
    }
}

#[test]
fn letter_after_float_literal() {
    assert_eq!(
        lex_with_errors("1ee 1.5x 1e1e").0,
        vec![
            TokenStr("1", LiteralFloat),
            TokenStr("ee", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("1.5", LiteralFloat),
            TokenStr("x", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("1e1", LiteralFloat),
            TokenStr("e", Identifier),
        ]
    );
    assert_eq!(
        errors("1ee 1.5x 1e1e"),
        expected_errors(&[
            ("1e", "invalid literal number"),
            ("1.5x", "invalid literal number"),
            ("1e1e", "invalid literal number"),
        ])
    );
}

#[test]
fn annotations() {
    let tokens = lex(r#"@ Pre annotation
        Some Identifiers @< Post annotation"#);

    assert_eq!(
        tokens,
        vec![
            TokenStr("@ Pre annotation", PreAnnotation),
            TokenStr("\n", Eol),
            TokenStr("        ", Whitespace),
            TokenStr("Some", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("Identifiers", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("@< Post annotation", PostAnnotation),
        ]
    );
}

#[test]
fn identifiers_and_keywords() {
    let tokens = lex(r#"Ident _underscope_start with_numbers01_asd yellow $yellow action every"#);

    assert_eq!(
        tokens,
        vec![
            TokenStr("Ident", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("_underscope_start", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("with_numbers01_asd", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("yellow", Keyword(KeywordKind::Yellow)),
            TokenStr(" ", Whitespace),
            TokenStr("$yellow", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("action", Keyword(KeywordKind::Action)),
            TokenStr(" ", Whitespace),
            TokenStr("every", Keyword(KeywordKind::Every)),
        ]
    );
}

#[test]
fn keyword_spellings() {
    for (text, kind) in crate::token::KEYWORDS {
        assert_eq!(lex(text), vec![TokenStr(text, Keyword(kind))]);
        assert_eq!(kind.as_str(), text);
        assert_eq!(KeywordKind::get(text), Some(kind));
    }

    assert_eq!(KeywordKind::U32.to_string(), "U32");
    assert_eq!(KeywordKind::String_.to_string(), "string");
    assert_eq!(KeywordKind::get("u32"), None);
}

#[test]
fn escape_newline() {
    let tokens = lex("escaped \\  \n    newline");
    assert_eq!(
        tokens,
        vec![
            TokenStr("escaped", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("\\  \n", Whitespace),
            TokenStr("    ", Whitespace),
            TokenStr("newline", Identifier),
        ]
    );
}

#[test]
fn escape_newline_error() {
    let (tokens, _) = lex_with_errors("escaped \\hello\n    newline");
    assert_eq!(
        tokens,
        vec![
            TokenStr("escaped", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("\\hello\n", Whitespace),
            TokenStr("    ", Whitespace),
            TokenStr("newline", Identifier),
        ]
    );
    assert_eq!(
        errors("escaped \\hello\n    newline"),
        expected_errors(&[("hello", "expected line continuation")])
    );

    // A backslash that ends the input
    assert_eq!(
        errors("a \\"),
        expected_errors(&[("\\", "expected line continuation")])
    );
}

#[test]
fn invalid_tokens() {
    let content = "\t $ ™™ < > %";
    let (tokens, _) = lex_with_errors(content);
    assert_eq!(
        tokens,
        vec![
            TokenStr("\t", Unknown),
            TokenStr(" ", Whitespace),
            TokenStr("$", Unknown),
            TokenStr(" ", Whitespace),
            TokenStr("™", Unknown),
            TokenStr("™", Unknown),
            TokenStr(" ", Whitespace),
            TokenStr("<", Unknown),
            TokenStr(" ", Whitespace),
            TokenStr(">", Unknown),
            TokenStr(" ", Whitespace),
            TokenStr("%", Unknown),
        ]
    );
    assert_eq!(
        errors(content),
        expected_errors(&[
            ("\t", "unicode value 9, hex 0x9"),
            ("$", "invalid usage of '$', expected identifier"),
            ("™", "unicode value 8482, hex 0x2122"),
            ("™", "unicode value 8482, hex 0x2122"),
            ("<", "'<'"),
            (">", "'>'"),
            ("%", "unicode value 37, hex 0x25"),
        ])
    );

    let mut lexer = crate::Lexer::new("\t");
    lexer.by_ref().for_each(drop);
    assert_eq!(
        lexer.errors().next().unwrap().notes,
        vec![
            "embedded tab characters are not allowed in FPP source files",
            "try configuring your editor to convert tabs to spaces",
        ]
    );
}

#[test]
fn unclosed_strings() {
    let content = "\"abc\n    \"\n\"de\\\nf\"";
    let (tokens, _) = lex_with_errors(content);
    assert_eq!(
        tokens,
        vec![
            TokenStr("\"abc", LiteralString),
            TokenStr("\n", Eol),
            TokenStr("    ", Whitespace),
            TokenStr("\"", LiteralString),
            TokenStr("\n", Eol),
            TokenStr("\"de\\", LiteralString),
            TokenStr("\n", Eol),
            TokenStr("f", Identifier),
            TokenStr("\"", LiteralString),
        ]
    );
    assert_eq!(
        errors(content),
        expected_errors(&[
            ("\"abc", "missing string double quote closure"),
            ("\"", "missing string double quote closure"),
            ("\"de\\", "missing string double quote closure"),
            ("\"", "missing string double quote closure"),
        ])
    );

    let content = "\"\"\" asdaldkasl\nmore";
    assert_eq!(
        lex_with_errors(content).0,
        vec![TokenStr(content, LiteralMultilineString { indent: 1 })]
    );
    assert_eq!(
        errors(content),
        expected_errors(&[("\"\"\"", "unclosed multi-line string literal")])
    );
}

/// The spec allows no whitespace character other than the space
#[test]
fn form_feed_is_not_whitespace() {
    assert_eq!(
        errors("a\u{c}b"),
        expected_errors(&[("\u{c}", "unicode value 12, hex 0xc")])
    );
}

/// An end-of-input character in the source is an invalid character
#[test]
fn substitute_character_is_invalid() {
    assert_eq!(
        errors("a\u{1a}b"),
        expected_errors(&[("\u{1a}", "unicode value 26, hex 0x1a")])
    );
}

#[test]
fn crlf_is_a_newline() {
    let content = "@ a\r\n# b\r\nc \\\r\nd\r\n\r\n\"\"\"\r\n  e\r\n  \"\"\"";
    assert_eq!(
        lex(content),
        vec![
            TokenStr("@ a", PreAnnotation),
            TokenStr("\r\n", Eol),
            TokenStr("# b", Comment),
            TokenStr("\r\n", Eol),
            TokenStr("c", Identifier),
            TokenStr(" ", Whitespace),
            TokenStr("\\\r\n", Whitespace),
            TokenStr("d", Identifier),
            TokenStr("\r\n\r\n", Eol),
            TokenStr(
                "\"\"\"\r\n  e\r\n  \"\"\"",
                LiteralMultilineString { indent: 2 }
            ),
        ]
    );

    // A carriage return on its own is an invalid character
    assert_eq!(
        errors("a\rb"),
        expected_errors(&[("\r", "unicode value 13, hex 0xd")])
    );
}

#[test]
fn symbols() {
    let tokens = lex(r#": . , = () ) {} } [] ] -> - + ; / * << >>"#);

    assert_eq!(
        tokens
            .into_iter()
            .filter(|t| t.1 != Whitespace)
            .collect::<Vec<_>>(),
        vec![
            TokenStr(":", Colon),
            TokenStr(".", Dot),
            TokenStr(",", Comma),
            TokenStr("=", Equals),
            TokenStr("(", LeftParen),
            TokenStr(")", RightParen),
            TokenStr(")", RightParen),
            TokenStr("{", LeftCurly),
            TokenStr("}", RightCurly),
            TokenStr("}", RightCurly),
            TokenStr("[", LeftSquare),
            TokenStr("]", RightSquare),
            TokenStr("]", RightSquare),
            TokenStr("->", RightArrow),
            TokenStr("-", Minus),
            TokenStr("+", Plus),
            TokenStr(";", Semi),
            TokenStr("/", Slash),
            TokenStr("*", Star),
            TokenStr("<<", ShiftLeft),
            TokenStr(">>", ShiftRight),
        ]
    );
}
