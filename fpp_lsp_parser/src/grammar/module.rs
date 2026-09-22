use super::*;

pub(super) fn def_module(p: &mut Parser) {
    assert!(p.at(MODULE_KW));
    let m = p.start();
    p.bump(MODULE_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }
    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            module_member,
            SEMI,
            MODULE_MEMBER_LIST,
            "expected module member",
        )
    } else {
        p.error("expected '{'")
    }

    m.complete(p, DEF_MODULE);
}

pub(super) fn module_member(p: &mut Parser) {
    match p.current() {
        DICTIONARY_KW if p.nth_at(1, TYPE_KW) => types::type_alias_or_abstract(p),
        TYPE_KW => types::type_alias_or_abstract(p),
        DICTIONARY_KW if p.nth_at(1, ARRAY_KW) => types::def_array(p),
        ARRAY_KW => types::def_array(p),
        DICTIONARY_KW if p.nth_at(1, CONSTANT_KW) => module::def_constant(p),
        CONSTANT_KW => def_constant(p),
        DICTIONARY_KW if p.nth_at(1, ENUM_KW) => types::def_enum(p),
        ENUM_KW => types::def_enum(p),
        DICTIONARY_KW if p.nth_at(1, STRUCT_KW) => types::def_struct(p),
        STRUCT_KW => types::def_struct(p),

        INSTANCE_KW => def_component_instance(p),
        PASSIVE_KW | ACTIVE_KW | QUEUED_KW => component::def_component(p),
        INTERFACE_KW => def_interface(p),
        MODULE_KW => def_module(p),
        PORT_KW => def_port(p),
        STATE_KW => state_machine::def_state_machine(p),
        SYSTEM_KW => topology::def_system(p),
        TOPOLOGY_KW | DEPLOYMENT_KW => topology::def_topology(p),
        INCLUDE_KW => spec_include(p),
        LOCATE_KW => spec_loc(p),

        _ => {
            p.err_and_bump("module member expected");
        }
    }
}

fn spec_loc(p: &mut Parser) {
    assert!(p.at(LOCATE_KW));
    let m = p.start();
    p.bump(LOCATE_KW);
    match p.current() {
        COMPONENT_KW | CONSTANT_KW | INSTANCE_KW | PORT_KW | TYPE_KW | INTERFACE_KW | SYSTEM_KW => {
            p.bump_any();
        }
        STATE_KW => {
            p.bump(STATE_KW);
            p.expect(MACHINE_KW);
        }
        _ => {
            m.abandon(p);
            p.err_recover("expected locate specifier", MEMBER_RECOVERY_SET);
            return;
        }
    }

    qual_ident(p);
    p.expect(AT_KW);
    p.expect(LITERAL_STRING);

    m.complete(p, SPEC_LOC);
}

fn def_component_instance(p: &mut Parser) {
    assert!(p.at(INSTANCE_KW));
    let m = p.start();
    p.bump(INSTANCE_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    p.expect(COLON);
    qual_ident(p);

    {
        let m = p.start();
        p.expect(BASE_KW);
        p.expect(ID_KW);
        expr::expr(p);
        m.complete(p, BASE_ID);
    }

    if p.at(TYPE_KW) {
        let m = p.start();
        p.bump(TYPE_KW);
        p.expect(LITERAL_STRING);
        m.complete(p, COMPONENT_INSTANCE_TYPE);
    }

    if p.at(AT_KW) {
        let m = p.start();
        p.bump(AT_KW);
        p.expect(LITERAL_STRING);
        m.complete(p, COMPONENT_INSTANCE_FILE);
    }

    if p.at(QUEUE_KW) {
        let m = p.start();
        p.bump(QUEUE_KW);
        p.expect(SIZE_KW);
        expr::expr(p);
        m.complete(p, QUEUE_SIZE);
    }

    if p.at(STACK_KW) {
        let m = p.start();
        p.bump(STACK_KW);
        p.expect(SIZE_KW);
        expr::expr(p);
        m.complete(p, STACK_SIZE);
    }

    expr_opt(p, PRIORITY_KW, PRIORITY);
    expr_opt(p, CPU_KW, CPU);

    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            spec_init,
            SEMI,
            INIT_SPEC_LIST,
            "expected initialization specifier",
        )
    }

    m.complete(p, DEF_COMPONENT_INSTANCE);
}

fn spec_init(p: &mut Parser) {
    let m = p.start();
    p.expect(PHASE_KW);
    expr::expr(p);
    p.expect(LITERAL_STRING);

    m.complete(p, SPEC_INIT);
}

fn def_interface(p: &mut Parser) {
    assert!(p.at(INTERFACE_KW));
    let m = p.start();
    p.bump(INTERFACE_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            interface_member,
            SEMI,
            INTERFACE_MEMBER_LIST,
            "expected interface member",
        )
    }

    m.complete(p, DEF_INTERFACE);
}

fn interface_member(p: &mut Parser) {
    if p.at(IMPORT_KW) {
        spec_import_interface(p);
    } else {
        component::spec_port_instance(p);
    }
}

pub(super) fn def_constant(p: &mut Parser) {
    assert!(p.at(CONSTANT_KW) || p.at(DICTIONARY_KW));
    let m = p.start();
    p.eat(DICTIONARY_KW);
    p.expect(CONSTANT_KW);

    name_r(p, MEMBER_RECOVERY_SET);

    if p.eat(EQUALS) {
        expr::expr(p);
    } else {
        p.error("expected `=`");
    }

    m.complete(p, DEF_CONSTANT);
}

pub(super) fn spec_import_interface(p: &mut Parser) {
    assert!(p.at(IMPORT_KW));
    let m = p.start();
    p.bump(IMPORT_KW);
    qual_ident(p);
    m.complete(p, SPEC_INTERFACE_IMPORT);
}

fn def_port(p: &mut Parser) {
    assert!(p.at(PORT_KW));
    let m = p.start();
    p.bump(PORT_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }
    formal_param_list(p);

    if p.eat(RIGHT_ARROW) {
        types::type_name(p);
    }

    m.complete(p, DEF_PORT);
}
