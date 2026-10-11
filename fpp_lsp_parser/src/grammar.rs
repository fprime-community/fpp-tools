mod component;
pub(crate) mod entry;
mod expr;
mod module;
mod state_machine;
mod topology;
mod types;

use crate::token_set::TokenSet;
use crate::{SyntaxKind, SyntaxKind::*, parser::Parser};

pub(super) const MEMBER_RECOVERY_SET: TokenSet = TokenSet::new(&[
    EOL,
    SEMI,
    RIGHT_CURLY,
    // TYPE_KW,
    // ARRAY_KW,
    // ASYNC_KW,
    // GUARDED_KW,
    // SYNC_KW,
    // OUTPUT_KW,
    // COMPONENT_KW,
    // INSTANCE_KW,
    // CONSTANT_KW,
    // ENUM_KW,
    // INTERFACE_KW,
    // MODULE_KW,
    // PORT_KW,
    // STATE_KW,
    // STRUCT_KW,
    // TOPOLOGY_KW,
    // INCLUDE_KW,
    // LOCATE_KW,
]);

/// Parse the name of a definition, or report that it is missing and recover
/// at a token in `recovery`.
fn name_r(p: &mut Parser<'_>, recovery: TokenSet) {
    if p.at(IDENT) {
        let m = p.start();
        p.bump(IDENT);
        m.complete(p, NAME);
    } else {
        p.err_recover("identifier expected", recovery);
    }
}

/// Parse the name of a definition, or report that it is missing. Returns
/// whether the name is present.
fn name(p: &mut Parser<'_>) -> bool {
    if p.at(IDENT) {
        let m = p.start();
        p.bump(IDENT);
        m.complete(p, NAME);
        true
    } else {
        p.error("identifier expected");
        false
    }
}

fn name_ref_r(p: &mut Parser<'_>, recovery: TokenSet) {
    if p.at(IDENT) {
        let m = p.start();
        p.bump(IDENT);
        m.complete(p, NAME_REF);
    } else {
        p.err_recover("identifier expected", recovery);
    }
}

fn name_ref(p: &mut Parser<'_>) {
    name_ref_r(p, TokenSet::EMPTY);
}

fn error_block(p: &mut Parser<'_>, message: &str) {
    assert!(p.at(LEFT_CURLY));
    let m = p.start();
    p.error(message);
    p.bump(LEFT_CURLY);
    while !p.at(EOF) && !p.at(RIGHT_CURLY) {
        p.bump_any();
    }
    p.eat(RIGHT_CURLY);
    m.complete(p, ERROR);
}

/// Whether the parser is at the start of a name, which may be absolute
/// (written with a leading dot).
fn at_name_start(p: &Parser) -> bool {
    p.at(IDENT) || (p.at(DOT) && p.nth_at(1, IDENT))
}

/// Parse a qualified identifier, optionally written with a leading dot that
/// makes it absolute (Scala `qualIdent`: `opt(dot) ~ qualIdentNodeList`).
pub(super) fn qual_ident(p: &mut Parser) {
    if !at_name_start(p) {
        p.expect(QUAL_IDENT);
        return;
    }

    let m = p.start();
    p.eat(DOT);
    qual_ident_node_list(p);
    m.complete(p, QUAL_IDENT);
}

/// Parse identifiers separated by dots (Scala `qualIdentNodeList`).
fn qual_ident_node_list(p: &mut Parser) {
    p.expect(IDENT);
    while p.at(DOT) {
        p.bump(DOT);
        if !p.expect(IDENT) {
            break;
        }
    }
}

/// Whether a sequence of `kind` is an annotated element sequence (Scala
/// `annotatedElementSequence`), whose elements may carry pre-annotations and
/// post-annotations, rather than a plain element sequence (Scala
/// `elementSequence`), which admits no annotations.
fn is_annotated_sequence(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        ROOT | MODULE_MEMBER_LIST
            | COMPONENT_MEMBER_LIST
            | INTERFACE_MEMBER_LIST
            | STATE_MACHINE_MEMBER_LIST
            | STATE_MEMBER_LIST
            | TOPOLOGY_MEMBER_LIST
            | TLM_PACKET_SET_MEMBER_LIST
            | ENUM_MEMBER_LIST
            | STRUCT_MEMBER_LIST
            | FORMAL_PARAM_LIST
            | TEMPLATE_PARAM_LIST
            | INIT_SPEC_LIST
    )
}

/// The source spelling of a closing delimiter, for diagnostics.
fn closing_text(ket: SyntaxKind) -> &'static str {
    match ket {
        RIGHT_CURLY => "}",
        RIGHT_PAREN => ")",
        RIGHT_SQUARE => "]",
        _ => unreachable!("not a closing delimiter: {:?}", ket),
    }
}

/// Parse a bracketed sequence of members into a node of kind `list_kind`.
fn member_list(
    p: &mut Parser,
    bra: SyntaxKind,
    ket: SyntaxKind,
    member: impl Fn(&mut Parser),
    delim: SyntaxKind,
    list_kind: SyntaxKind,
    expected_error_msg: &'static str,
) {
    let m = p.start();

    assert!(p.at(bra));
    p.bump(bra);

    element_sequence(
        p,
        ket,
        member,
        delim,
        is_annotated_sequence(list_kind),
        expected_error_msg,
    );

    p.expect(ket);
    m.complete(p, list_kind);
}

/// Parse the members of a sequence up to `ket`, which is `EOF` for the
/// members that make up a whole file.
///
/// The elements are separated or terminated by `delim` or by an end of
/// line, and the last one may be unterminated (Scala `elementSequence`). In
/// an annotated sequence (Scala `annotatedElementSequence`) the sequence may
/// begin with end-of-line tokens, an element may be preceded by
/// pre-annotations, and a terminated element may be followed by
/// post-annotations, which also terminate it. Annotations that are not
/// followed by an element are an error, as is any annotation in a plain
/// sequence. A plain sequence with no elements may consist of one
/// terminator.
fn element_sequence(
    p: &mut Parser,
    ket: SyntaxKind,
    member: impl Fn(&mut Parser),
    delim: SyntaxKind,
    annotated: bool,
    expected_error_msg: &'static str,
) {
    let at_end = |p: &Parser| p.at(ket) || p.at(EOF);
    // What Scala reports when an unterminated element is not the last one
    let unterminated_msg = if ket == EOF {
        "unexpected token".to_string()
    } else {
        format!("{} expected", closing_text(ket))
    };

    if annotated {
        while p.at(EOL) {
            p.bump(EOL);
        }
    } else if p.at(delim) || p.at(EOL) {
        p.bump_any();
        if !at_end(p) {
            p.error(unterminated_msg.as_str());
        }
    }

    while !at_end(p) {
        if ket == RIGHT_CURLY && p.at(LEFT_CURLY) {
            error_block(p, expected_error_msg);
            continue;
        }

        let start = p.position();

        if annotated && p.at(PRE_ANNOTATION) {
            while p.at(PRE_ANNOTATION) {
                p.bump(PRE_ANNOTATION);
            }
            if at_end(p) {
                // Pre-annotations must be followed by an element
                p.error(expected_error_msg);
                break;
            }
        }

        member(p);

        if p.eat(delim) || p.eat(EOL) || (annotated && p.at(POST_ANNOTATION)) {
            if annotated {
                while p.at(POST_ANNOTATION) {
                    p.bump(POST_ANNOTATION);
                }
            }
        } else if !at_end(p) {
            // Only the last element may be unterminated
            p.err_recover(&unterminated_msg, MEMBER_RECOVERY_SET);
        }

        // Every iteration must consume input. A closing delimiter that does
        // not close this sequence closes an enclosing construct.
        if p.position() == start {
            if ket != EOF && (p.at(RIGHT_CURLY) || p.at(RIGHT_PAREN) || p.at(RIGHT_SQUARE)) {
                break;
            }
            p.err_and_bump(expected_error_msg);
        }
    }
}

fn expr_opt(p: &mut Parser, prefix: SyntaxKind, rule: SyntaxKind) {
    if p.at(prefix) {
        let m = p.start();
        p.bump(prefix);
        expr::expr(p);
        m.complete(p, rule);
    }
}

fn index_or_size(p: &mut Parser) {
    assert!(p.at(LEFT_SQUARE));
    let m = p.start();
    p.bump(LEFT_SQUARE);
    expr::expr(p);
    p.expect(RIGHT_SQUARE);
    m.complete(p, INDEX_OR_SIZE);
}

fn index_or_size_opt(p: &mut Parser) {
    if p.at(LEFT_SQUARE) {
        index_or_size(p);
    }
}

fn formal_param_list(p: &mut Parser) {
    if p.at(LEFT_PAREN) {
        member_list(
            p,
            LEFT_PAREN,
            RIGHT_PAREN,
            formal_param,
            COMMA,
            FORMAL_PARAM_LIST,
            "expected formal parameter",
        );
    }
}

fn formal_param(p: &mut Parser) {
    let m = p.start();
    p.eat(REF_KW);
    if p.at(IDENT) {
        name(p);

        if p.eat(COLON) {
            types::type_name(p);
        } else {
            p.error("expected `:`");
        }

        m.complete(p, FORMAL_PARAM);
    } else {
        m.abandon(p);
        p.err_and_bump("expected formal parameter");
    }
}

fn format_(p: &mut Parser) {
    let m = p.start();
    p.expect(FORMAT_KW);
    p.expect(LITERAL_STRING);
    m.complete(p, FORMAT);
}

fn format_opt(p: &mut Parser) {
    if p.at(FORMAT_KW) {
        format_(p);
    }
}

fn spec_include(p: &mut Parser) {
    assert!(p.at(INCLUDE_KW));
    let m = p.start();
    p.bump(INCLUDE_KW);
    p.expect(LITERAL_STRING);
    m.complete(p, SPEC_INCLUDE);
}
