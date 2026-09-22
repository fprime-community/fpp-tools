use crate::SyntaxKind::*;
use crate::parser::Parser;

use super::*;

pub(super) fn type_alias_or_abstract(p: &mut Parser) {
    assert!(p.at(TYPE_KW) || p.at(DICTIONARY_KW));
    let m = p.start();
    p.eat(DICTIONARY_KW);
    p.expect(TYPE_KW);
    name_r(p, MEMBER_RECOVERY_SET);
    if p.at(EQUALS) {
        p.bump(EQUALS);
        type_name(p);
        m.complete(p, DEF_ALIAS_TYPE);
    } else {
        m.complete(p, DEF_ABSTRACT_TYPE);
    }
}

pub(super) fn def_array(p: &mut Parser) {
    assert!(p.at(ARRAY_KW) || p.at(DICTIONARY_KW));
    let m = p.start();
    p.eat(DICTIONARY_KW);
    p.expect(ARRAY_KW);
    name_r(p, MEMBER_RECOVERY_SET);

    if !p.eat(EQUALS) {
        p.error("expected `=`");
    }

    if p.at(LEFT_SQUARE) {
        index_or_size(p);
    } else {
        p.err_and_bump("expected `[`")
    }

    type_name(p);
    expr_opt(p, DEFAULT_KW, DEFAULT);
    format_opt(p);

    m.complete(p, DEF_ARRAY);
}

pub(super) fn def_struct(p: &mut Parser) {
    assert!(p.at(STRUCT_KW) || p.at(DICTIONARY_KW));

    let m = p.start();
    p.eat(DICTIONARY_KW);
    p.expect(STRUCT_KW);
    name_r(p, MEMBER_RECOVERY_SET);

    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            struct_field,
            COMMA,
            STRUCT_MEMBER_LIST,
            "expected struct member",
        );
    } else {
        p.error("expected `{`");
    }

    expr_opt(p, DEFAULT_KW, DEFAULT);
    m.complete(p, DEF_STRUCT);
}

pub(super) fn struct_field(p: &mut Parser) {
    let m = p.start();
    if p.at(IDENT) {
        name(p);
        p.expect(COLON);

        index_or_size_opt(p);
        type_name(p);
        format_opt(p);

        m.complete(p, STRUCT_MEMBER);
    } else {
        m.abandon(p);
        p.err_and_bump("expected struct field declaration");
    }
}

pub(super) fn def_enum(p: &mut Parser) {
    assert!(p.at(ENUM_KW) || p.at(DICTIONARY_KW));
    let m = p.start();
    p.eat(DICTIONARY_KW);
    p.expect(ENUM_KW);

    name_r(p, MEMBER_RECOVERY_SET);

    if p.at(COLON) {
        p.bump(COLON);
        type_name(p);
    }

    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            enum_member,
            COMMA,
            ENUM_MEMBER_LIST,
            "expected enum member",
        );
    } else {
        p.error("expected `{`");
    }

    expr_opt(p, DEFAULT_KW, DEFAULT);
    m.complete(p, DEF_ENUM);
}

pub(super) fn enum_member(p: &mut Parser) {
    let m = p.start();
    if p.at(IDENT) {
        name(p);

        if p.eat(EQUALS) {
            expr::expr(p);
        }
        m.complete(p, DEF_ENUM_CONSTANT);
    } else {
        m.abandon(p);
        p.err_and_bump("expected enum variant");
    }
}

pub(super) fn type_name(p: &mut Parser) {
    let m = p.start();
    match p.current() {
        BOOL_KW | I8_KW | U8_KW | I16_KW | U16_KW | I32_KW | U32_KW | I64_KW | U64_KW | F32_KW
        | F64_KW => {
            p.bump(p.current());
        }
        STRING_KW => {
            p.bump(STRING_KW);
            if p.eat(SIZE_KW) {
                expr::expr(p)
            }
        }
        IDENT => qual_ident(p),
        _ => {
            m.abandon(p);
            p.expect(TYPE_NAME);
            return;
        }
    }

    m.complete(p, TYPE_NAME);
}
