use super::*;

const STATE_MACHINE_MEMBER_RECOVERY_SET: TokenSet = TokenSet::new(&[
    EOL,
    SEMI,
    STATE_KW,
    SIGNAL_KW,
    INITIAL_KW,
    ACTION_KW,
    GUARD_KW,
    CHOICE_KW,
    TYPE_KW,
    ARRAY_KW,
    CONSTANT_KW,
    ENUM_KW,
    STRUCT_KW,
    INCLUDE_KW,
]);

const STATE_MEMBER_RECOVERY_SET: TokenSet = TokenSet::new(&[
    EOL, SEMI, STATE_KW, CHOICE_KW, INITIAL_KW, ENTRY_KW, EXIT_KW, ON_KW, INCLUDE_KW,
]);

pub(super) fn def_state_machine(p: &mut Parser) {
    assert!(p.at(STATE_KW));
    let m = p.start();
    p.bump(STATE_KW);
    p.expect(MACHINE_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }
    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            state_machine_member,
            SEMI,
            STATE_MACHINE_MEMBER_LIST,
            "expected state machine member",
        )
    }

    m.complete(p, DEF_STATE_MACHINE);
}

fn state_machine_member(p: &mut Parser) {
    match p.current() {
        TYPE_KW => types::type_alias_or_abstract(p),
        ARRAY_KW => types::def_array(p),
        CONSTANT_KW => module::def_constant(p),
        ENUM_KW => types::def_enum(p),
        STRUCT_KW => types::def_struct(p),
        INCLUDE_KW => spec_include(p),
        INITIAL_KW => spec_initial_transition(p),
        STATE_KW => def_state(p),
        SIGNAL_KW => def_signal(p),
        ACTION_KW => def_action(p),
        GUARD_KW => def_guard(p),
        CHOICE_KW => def_choice(p),
        _ => {
            p.err_recover(
                "expected state machine member",
                STATE_MACHINE_MEMBER_RECOVERY_SET,
            );
        }
    }
}

fn spec_initial_transition(p: &mut Parser) {
    assert!(p.at(INITIAL_KW));
    let m = p.start();
    p.bump(INITIAL_KW);
    transition_expr(p);
    m.complete(p, SPEC_INITIAL_TRANSITION);
}

fn def_state(p: &mut Parser) {
    assert!(p.at(STATE_KW));
    let m = p.start();
    p.bump(STATE_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            state_member,
            SEMI,
            STATE_MEMBER_LIST,
            "expected state member",
        );
    }

    m.complete(p, DEF_STATE);
}

fn state_member(p: &mut Parser) {
    match p.current() {
        CHOICE_KW => def_choice(p),
        STATE_KW => def_state(p),
        INITIAL_KW => spec_initial_transition(p),
        ENTRY_KW => spec_state_entry(p),
        EXIT_KW => spec_state_exit(p),
        INCLUDE_KW => spec_include(p),
        ON_KW => spec_state_transition(p),
        _ => {
            p.err_recover("expected state machine member", STATE_MEMBER_RECOVERY_SET);
        }
    }
}

fn def_signal(p: &mut Parser) {
    assert!(p.at(SIGNAL_KW));
    let m = p.start();
    p.bump(SIGNAL_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    if p.eat(COLON) {
        types::type_name(p);
    }

    m.complete(p, DEF_SIGNAL);
}

fn def_action(p: &mut Parser) {
    assert!(p.at(ACTION_KW));
    let m = p.start();
    p.bump(ACTION_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    if p.eat(COLON) {
        types::type_name(p);
    }

    m.complete(p, DEF_ACTION);
}

fn def_guard(p: &mut Parser) {
    assert!(p.at(GUARD_KW));
    let m = p.start();
    p.bump(GUARD_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    if p.eat(COLON) {
        types::type_name(p);
    }

    m.complete(p, DEF_GUARD);
}

fn def_choice(p: &mut Parser) {
    assert!(p.at(CHOICE_KW));
    let m = p.start();
    p.bump(CHOICE_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    p.expect(LEFT_CURLY);

    // Skip any EOL tokens after the opening brace (formatter adds newline after {)
    while p.at(EOL) {
        p.bump_any();
    }

    p.expect(IF_KW);
    name_ref(p);

    {
        let m = p.start();
        transition_expr(p);
        m.complete(p, THEN_CLAUSE);
    }

    p.expect(ELSE_KW);
    {
        let m = p.start();
        transition_expr(p);
        m.complete(p, ELSE_CLAUSE);
    }

    // Skip EOL before closing brace
    while p.at(EOL) {
        p.bump_any();
    }

    p.expect(RIGHT_CURLY);

    m.complete(p, DEF_CHOICE);
}

fn spec_state_entry(p: &mut Parser) {
    assert!(p.at(ENTRY_KW));
    let m = p.start();
    p.bump(ENTRY_KW);
    if p.at(DO_KW) {
        do_expr(p);
    } else {
        p.error("expected do expression");
        m.abandon(p);
        return;
    }

    m.complete(p, SPEC_STATE_ENTRY);
}

fn spec_state_exit(p: &mut Parser) {
    assert!(p.at(EXIT_KW));
    let m = p.start();
    p.bump(EXIT_KW);
    if p.at(DO_KW) {
        do_expr(p);
    } else {
        p.error("expected do expression");
        m.abandon(p);
        return;
    }

    m.complete(p, SPEC_STATE_EXIT);
}

fn spec_state_transition(p: &mut Parser) {
    assert!(p.at(ON_KW));
    let m = p.start();
    p.bump(ON_KW);
    p.expect(IDENT);

    if p.eat(IF_KW) {
        name_ref(p);
    }

    transition_or_do(p);
    m.complete(p, SPEC_STATE_TRANSITION);
}

fn transition_or_do(p: &mut Parser) {
    let m = p.start();
    if p.at(DO_KW) {
        do_expr(p);

        if p.eat(ENTER_KW) {
            qual_ident(p);
            m.complete(p, TRANSITION_EXPR);
        } else {
            // Do expr
            m.abandon(p);
        }
    } else if p.eat(ENTER_KW) {
        qual_ident(p);
        m.complete(p, TRANSITION_EXPR);
    } else {
        m.abandon(p);
        p.error("expected transition or do expression")
    }
}

fn transition_expr(p: &mut Parser) {
    let m = p.start();

    if p.at(DO_KW) {
        do_expr(p);
    }

    p.expect(ENTER_KW);
    qual_ident(p);

    m.complete(p, TRANSITION_EXPR);
}

fn do_expr(p: &mut Parser) {
    assert!(p.at(DO_KW));
    let m = p.start();
    p.bump(DO_KW);

    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            name_ref,
            COMMA,
            DO_EXPR_MEMBER_LIST,
            "expected action name",
        )
    } else {
        p.error("expected '{'")
    }

    m.complete(p, DO_EXPR);
}
