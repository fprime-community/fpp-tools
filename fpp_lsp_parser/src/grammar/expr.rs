use super::*;
use crate::parser::CompletedMarker;

pub(super) fn expr(p: &mut Parser) {
    let m = p.start();
    let mut lhs = match expr_shift_operand(p) {
        None => {
            m.abandon(p);
            return;
        }
        Some(lhs) => lhs,
    };

    loop {
        match p.current() {
            SHIFT_LEFT | SHIFT_RIGHT => {
                let m = lhs.precede(p);
                let op = p.start();
                p.bump_any();
                op.complete(p, BINARY_OP);
                expr_shift_operand(p);
                lhs = m.complete(p, EXPR_BINARY);
            }
            _ => {
                m.complete(p, EXPR);
                return;
            }
        }
    }
}

fn expr_shift_operand(p: &mut Parser) -> Option<CompletedMarker> {
    let mut lhs = expr_add_sub_operand(p)?;

    loop {
        match p.current() {
            PLUS | MINUS => {
                let m = lhs.precede(p);
                let op = p.start();
                p.bump_any();
                op.complete(p, BINARY_OP);
                expr_add_sub_operand(p);
                lhs = m.complete(p, EXPR_BINARY);
            }
            _ => return Some(lhs),
        }
    }
}

fn expr_add_sub_operand(p: &mut Parser) -> Option<CompletedMarker> {
    let mut lhs = expr_mul_div_operand(p)?;

    loop {
        match p.current() {
            STAR | SLASH => {
                let m = lhs.precede(p);
                let op = p.start();
                p.bump_any();
                op.complete(p, BINARY_OP);
                expr_mul_div_operand(p);
                lhs = m.complete(p, EXPR_BINARY);
            }
            _ => return Some(lhs),
        }
    }
}

fn expr_mul_div_operand(p: &mut Parser) -> Option<CompletedMarker> {
    let m = p.start();
    if p.eat(MINUS) {
        expr_postfix(p);
        Some(m.complete(p, EXPR_UNARY))
    } else {
        m.abandon(p);
        expr_postfix(p)
    }
}

fn expr_postfix(p: &mut Parser) -> Option<CompletedMarker> {
    let m_postfix = p.start();
    let mut found = false;

    let mut lhs = match expr_primary(p) {
        None => {
            m_postfix.abandon(p);
            return None;
        }
        Some(lhs) => lhs,
    };

    loop {
        match p.current() {
            DOT => {
                found = true;

                let m = lhs.precede(p);
                p.bump(DOT);
                p.expect(IDENT);
                lhs = m.complete(p, EXPR_MEMBER);
            }
            LEFT_SQUARE => {
                found = true;

                let m = lhs.precede(p);
                index_or_size(p);
                lhs = m.complete(p, EXPR_SUBSCRIPT);
            }
            _ => {
                return {
                    if found {
                        Some(m_postfix.complete(p, EXPR_POSTFIX))
                    } else {
                        m_postfix.abandon(p);
                        Some(lhs)
                    }
                };
            }
        }
    }
}

fn expr_primary(p: &mut Parser) -> Option<CompletedMarker> {
    match p.current() {
        LEFT_SQUARE => Some(expr_array(p)),
        FALSE_KW | TRUE_KW | LITERAL_FLOAT | LITERAL_INT | LITERAL_STRING => {
            let m = p.start();
            p.bump_any();
            Some(m.complete(p, EXPR_LITERAL))
        }
        LEFT_PAREN => Some(expr_paren(p)),
        LEFT_CURLY => Some(expr_struct(p)),
        IDENT => {
            let m = p.start();
            p.bump(IDENT);
            Some(m.complete(p, EXPR_IDENT))
        }
        // An absolute name
        DOT if p.nth_at(1, IDENT) => {
            let m = p.start();
            p.bump(DOT);
            p.bump(IDENT);
            Some(m.complete(p, EXPR_IDENT))
        }
        SIZEOF_KW => Some(expr_sizeof(p)),
        _ => {
            p.expect(EXPR);
            None
        }
    }
}

fn expr_sizeof(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(SIZEOF_KW));
    let m = p.start();
    p.bump(SIZEOF_KW);
    p.expect(LEFT_PAREN);
    types::type_name(p);
    p.expect(RIGHT_PAREN);
    m.complete(p, EXPR_SIZEOF)
}

fn expr_paren(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(LEFT_PAREN));
    let m = p.start();
    p.bump(LEFT_PAREN);
    expr(p);
    p.expect(RIGHT_PAREN);
    m.complete(p, EXPR)
}

fn expr_array(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(LEFT_SQUARE));
    let m = p.start();
    member_list(
        p,
        LEFT_SQUARE,
        RIGHT_SQUARE,
        expr,
        COMMA,
        EXPR_ARRAY_MEMBER_LIST,
        "expression expected",
    );

    m.complete(p, EXPR_ARRAY)
}

fn expr_struct(p: &mut Parser) -> CompletedMarker {
    assert!(p.at(LEFT_CURLY));
    let m = p.start();
    member_list(
        p,
        LEFT_CURLY,
        RIGHT_CURLY,
        expr_struct_member,
        COMMA,
        EXPR_STRUCT_MEMBER_LIST,
        "identifier expected",
    );

    m.complete(p, EXPR_STRUCT)
}

const EXPR_STRUCT_RECOVER: TokenSet = TokenSet::new(&[EOL, COMMA, RIGHT_CURLY, IDENT]);

fn expr_struct_member(p: &mut Parser) {
    if !p.at(IDENT) {
        p.err_recover("identifier expected", EXPR_STRUCT_RECOVER);
        return;
    }
    let m = p.start();
    name(p);

    p.expect(EQUALS);
    expr(p);
    m.complete(p, EXPR_STRUCT_MEMBER);
}
