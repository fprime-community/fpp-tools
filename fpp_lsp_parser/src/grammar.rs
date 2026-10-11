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

fn name_r(p: &mut Parser<'_>, recovery: TokenSet) {
    if p.at(IDENT) {
        let m = p.start();
        p.bump(IDENT);
        m.complete(p, NAME);
    } else {
        p.err_recover("expected a name", recovery);
    }
}

fn name(p: &mut Parser<'_>) -> bool {
    let m = p.start();
    if p.eat(IDENT) {
        m.complete(p, NAME);
        true
    } else {
        m.abandon(p);
        false
    }
}

fn name_ref_r(p: &mut Parser<'_>, recovery: TokenSet) {
    if p.at(IDENT) {
        let m = p.start();
        p.bump(IDENT);
        m.complete(p, NAME_REF);
    } else {
        p.err_recover("expected a name", recovery);
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

pub(super) fn qual_ident(p: &mut Parser) {
    if p.current() != IDENT {
        p.expect(QUAL_IDENT);
        return;
    }

    let m = p.start();
    p.expect(IDENT);
    while p.at(DOT) {
        p.bump(DOT);
        if !p.expect(IDENT) {
            break;
        }
    }

    m.complete(p, QUAL_IDENT);
}

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

    while !p.at(ket) && !p.at(EOF) {
        if bra == LEFT_CURLY && p.at(bra) {
            error_block(p, expected_error_msg);
            continue;
        }

        // Eat up EOLs before items
        while p.at(EOL) {
            p.bump_any();
        }

        if p.at(ket) {
            break;
        }

        let start = p.position();
        member(p);

        // Check for end delim
        if !p.eat(delim) && !p.eat(EOL) && !p.at(ket) {
            p.err_recover(&format!("expected `{:?}`", delim), MEMBER_RECOVERY_SET);
        }

        // Every iteration must consume input. A closing delimiter that does
        // not close this list closes an enclosing construct.
        if p.position() == start {
            if p.at(RIGHT_CURLY) || p.at(RIGHT_PAREN) || p.at(RIGHT_SQUARE) {
                break;
            }
            p.err_and_bump(expected_error_msg);
        }
    }

    p.expect(ket);
    m.complete(p, list_kind);
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
