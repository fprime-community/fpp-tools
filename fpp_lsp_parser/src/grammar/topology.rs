use super::*;

pub(super) fn def_topology(p: &mut Parser) {
    assert!(p.at(TOPOLOGY_KW) || p.at(DEPLOYMENT_KW));
    let m = p.start();
    p.eat(DEPLOYMENT_KW);
    p.expect(TOPOLOGY_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    if p.at(IMPLEMENTS_KW) {
        let m = p.start();
        p.bump(IMPLEMENTS_KW);
        qual_ident(p);
        while p.at(COMMA) {
            p.bump(COMMA);
            qual_ident(p);
        }
        m.complete(p, IMPLEMENTS_CLAUSE);
    }

    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            topology_member,
            SEMI,
            TOPOLOGY_MEMBER_LIST,
            "expected topology member",
        );
    } else {
        p.error("expected `{`")
    }

    m.complete(p, DEF_TOPOLOGY);
}

pub(super) fn def_system(p: &mut Parser) {
    assert!(p.at(SYSTEM_KW));
    let m = p.start();
    p.bump(SYSTEM_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    p.expect(COLON);
    qual_ident(p);

    m.complete(p, DEF_SYSTEM);
}

pub(super) fn topology_member(p: &mut Parser) {
    match p.current() {
        IMPORT_KW | INSTANCE_KW => spec_instance(p),
        INCLUDE_KW => spec_include(p),
        PORT_KW => spec_top_port(p),
        TELEMETRY_KW if p.nth_at(1, PACKETS_KW) => spec_tlm_packet_set(p),
        COMMAND_KW | EVENT_KW | HEALTH_KW | PARAM_KW | TEXT_KW | TIME_KW | TELEMETRY_KW => {
            spec_connection_graph_pattern(p)
        }
        CONNECTIONS_KW => spec_connection_graph_direct(p),
        _ => {
            p.err_and_bump("topology member expected");
        }
    }
}

fn spec_instance(p: &mut Parser) {
    assert!(p.at(IMPORT_KW) || p.at(INSTANCE_KW));
    let m = p.start();
    p.bump_any();

    qual_ident(p);
    m.complete(p, SPEC_INSTANCE);
}

fn interface_instance_member(p: &mut Parser) {
    p.expect(IDENT);
    while p.at(DOT) {
        p.bump(DOT);
        if !p.expect(IDENT) {
            break;
        }
    }
}

fn port_instance_identifier(p: &mut Parser) {
    let m = p.start();
    interface_instance_member(p);
    m.complete(p, PORT_INSTANCE_IDENTIFIER);
}

fn spec_top_port(p: &mut Parser) {
    assert!(p.at(PORT_KW));
    let m = p.start();
    p.bump(PORT_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }
    if !p.expect(EQUALS) {
        m.abandon(p);
        return;
    }

    port_instance_identifier(p);
    m.complete(p, SPEC_TOP_PORT);
}

fn spec_tlm_packet_set(p: &mut Parser) {
    assert!(p.at(TELEMETRY_KW));
    let m = p.start();
    p.bump(TELEMETRY_KW);
    p.bump(PACKETS_KW);

    if !name(p) {
        m.abandon(p);
        return;
    }
    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            tlm_packet_set_member,
            COMMA,
            TLM_PACKET_SET_MEMBER_LIST,
            "expected telemetry packet set member",
        )
    } else {
        p.error("expected `{`")
    }

    if p.at(OMIT_KW) {
        let m = p.start();
        p.eat(OMIT_KW);
        if p.at(LEFT_CURLY) {
            member_list(
                p,
                LEFT_CURLY,
                RIGHT_CURLY,
                tlm_channel_identifier,
                COMMA,
                TLM_PACKET_OMIT_MEMBER_LIST,
                "expected telemetry channel identifier",
            );
        } else {
            p.error("expected `{`")
        }

        m.complete(p, TLM_PACKET_OMIT);
    }

    m.complete(p, TLM_PACKET_SET);
}

fn tlm_packet_set_member(p: &mut Parser) {
    match p.current() {
        INCLUDE_KW => spec_include(p),
        PACKET_KW => spec_tlm_packet(p),
        _ => {
            p.err_and_bump("topology member expected");
        }
    }
}

fn spec_tlm_packet(p: &mut Parser) {
    assert!(p.at(PACKET_KW));
    let m = p.start();
    p.bump(PACKET_KW);
    if !name(p) {
        m.abandon(p);
        return;
    }

    expr_opt(p, ID_KW, ID);

    {
        let m = p.start();
        p.expect(GROUP_KW);
        expr::expr(p);
        m.complete(p, GROUP);
    }

    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            tlm_packet_member,
            COMMA,
            TLM_PACKET_MEMBER_LIST,
            "expected telemetry packet member",
        );
    } else {
        p.error("expected `{`")
    }

    m.complete(p, SPEC_TLM_PACKET);
}

fn tlm_packet_member(p: &mut Parser) {
    match p.current() {
        INCLUDE_KW => spec_include(p),
        IDENT => tlm_channel_identifier(p),
        _ => {
            p.err_and_bump("telemetry packet member expected");
        }
    }
}

fn tlm_channel_identifier(p: &mut Parser) {
    let m = p.start();
    interface_instance_member(p);
    m.complete(p, TLM_CHANNEL_IDENTIFIER);
}

fn spec_connection_graph_pattern(p: &mut Parser) {
    let m = p.start();

    match p.current() {
        COMMAND_KW | EVENT_KW | HEALTH_KW | PARAM_KW | TELEMETRY_KW | TIME_KW => {
            p.bump_any();
        }
        TEXT_KW => {
            p.bump(TEXT_KW);
            p.expect(EVENT_KW);
        }
        _ => unreachable!(),
    }

    p.expect(CONNECTIONS_KW);
    p.expect(INSTANCE_KW);
    qual_ident(p);

    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            qual_ident,
            COMMA,
            PATTERN_TARGET_MEMBER_LIST,
            "expected instance identifier",
        )
    }

    m.complete(p, SPEC_CONNECTION_GRAPH_PATTERN);
}

fn spec_connection_graph_direct(p: &mut Parser) {
    assert!(p.at(CONNECTIONS_KW));
    let m = p.start();
    p.bump(CONNECTIONS_KW);

    if !name(p) {
        m.abandon(p);
        return;
    }
    if p.at(LEFT_CURLY) {
        member_list(
            p,
            LEFT_CURLY,
            RIGHT_CURLY,
            connection,
            COMMA,
            CONNECTION_MEMBER_LIST,
            "expected connection",
        );
    } else {
        p.error("expected `{`")
    }

    m.complete(p, SPEC_CONNECTION_GRAPH_DIRECT);
}

fn connection(p: &mut Parser) {
    let m = p.start();

    p.eat(UNMATCHED_KW);
    connection_endpoint(p, true);
    p.expect(RIGHT_ARROW);
    connection_endpoint(p, false);

    m.complete(p, CONNECTION);
}

fn connection_endpoint(p: &mut Parser, is_from: bool) {
    let m = p.start();
    port_instance_identifier(p);
    index_or_size_opt(p);

    m.complete(
        p,
        match is_from {
            true => CONNECTION_FROM,
            false => CONNECTION_TO,
        },
    );
}
