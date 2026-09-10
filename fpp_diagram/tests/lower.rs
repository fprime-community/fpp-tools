//! Integration tests: parse FPP source, run analysis, lower to IR + sprotty.

use fpp_analysis::{Analysis, add_state_enums, check_semantics};
use fpp_core::SourceFile;
use fpp_diagram::ir::{DiagramKind, Direction, PortKind};

/// Parse `src` and run full semantic analysis, invoking `f` with the resulting
/// [`Analysis`] while the compiler context is installed.
fn with_analysis<R>(src: &str, f: impl FnOnce(&Analysis) -> R) -> R {
    let mut sink = Vec::new();
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut sink));
    fpp_core::run(&mut ctx, || {
        let source = SourceFile::new("test.fpp", src.to_string());
        let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
        add_state_enums(&mut ast);
        let mut a = Analysis::new();
        let _ = check_semantics(&mut a, vec![&ast]);
        f(&a)
    })
}

/// A small but representative model: two components (one active, one passive)
/// wired through a port, all inside a topology. Uses only general ports so it is
/// self-contained (no `Fw.*` framework definitions needed).
const MODEL: &str = r#"
port P

passive component Producer {
    output port pOut: P
    sync input port unused: P
}

active component Consumer {
    async input port cIn: P
    guarded input port gIn: P
    output port cOut: P
}

instance prod: Producer base id 0x100
instance cons: Consumer base id 0x200 \
    queue size 10

topology Sys {
    instance prod
    instance cons

    connections C {
        prod.pOut -> cons.cIn
    }
}
"#;

#[test]
fn lowers_topology_nodes_and_edges() {
    with_analysis(MODEL, |a| {
        let diagram =
            fpp_diagram::lower(a, DiagramKind::Topology, "Sys").expect("topology Sys should lower");

        assert_eq!(diagram.kind, DiagramKind::Topology);
        assert_eq!(diagram.name, "Sys");

        let node_ids: Vec<&str> = diagram.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(node_ids.contains(&"prod"), "nodes: {node_ids:?}");
        assert!(node_ids.contains(&"cons"), "nodes: {node_ids:?}");

        let prod = diagram.nodes.iter().find(|n| n.id == "prod").unwrap();
        assert_eq!(prod.class_name.as_deref(), Some("Producer"));
        assert_eq!(prod.kind, fpp_diagram::ir::ComponentKind::Passive);

        // The output port is present and classified as an output.
        let p_out = prod.ports.iter().find(|p| p.name == "pOut").unwrap();
        assert_eq!(p_out.direction, Direction::Output);
        assert_eq!(p_out.kind, PortKind::Output);
        assert_eq!(p_out.id, "prod.pOut.0");
        assert_eq!(p_out.type_name.as_deref(), Some("P"));

        let cons = diagram.nodes.iter().find(|n| n.id == "cons").unwrap();
        let c_in = cons.ports.iter().find(|p| p.name == "cIn").unwrap();
        assert_eq!(c_in.direction, Direction::Input);
        assert_eq!(c_in.kind, PortKind::Async);

        // The single connection becomes one edge between the two ports.
        assert_eq!(diagram.edges.len(), 1, "edges: {:?}", diagram.edges);
        let edge = &diagram.edges[0];
        assert_eq!(edge.from_port, "prod.pOut.0");
        assert_eq!(edge.to_port, "cons.cIn.0");
        assert_eq!(edge.graph_name, "C");
        assert!(!edge.unmatched);
    });
}

#[test]
fn lowers_component_definition() {
    with_analysis(MODEL, |a| {
        let diagram = fpp_diagram::lower(a, DiagramKind::Component, "Consumer")
            .expect("component Consumer should lower");
        assert_eq!(diagram.kind, DiagramKind::Component);
        assert_eq!(diagram.nodes.len(), 1);
        let node = &diagram.nodes[0];
        assert_eq!(node.class_name, None);
        assert_eq!(node.kind, fpp_diagram::ir::ComponentKind::Active);
        // General port kinds are classified for styling.
        let g_in = node.ports.iter().find(|p| p.name == "gIn").unwrap();
        assert_eq!(g_in.kind, PortKind::Guarded);
        let c_in = node.ports.iter().find(|p| p.name == "cIn").unwrap();
        assert_eq!(c_in.kind, PortKind::Async);
        let c_out = node.ports.iter().find(|p| p.name == "cOut").unwrap();
        assert_eq!(c_out.kind, PortKind::Output);
        assert_eq!(c_out.direction, Direction::Output);
    });
}

#[test]
fn raw_direct_graphs_expose_group_names_for_codelens() {
    // The LSP `fpp/diagramElements` handler anchors each connection-group lens at
    // the group's `connections <name>` node from `raw_direct_graphs`. Verify the
    // resolved topology retains those graphs and their names align with the
    // resolved `connection_map` keys.
    with_analysis(MODEL, |a| {
        use fpp_analysis::semantics::SymbolInterface;
        use fpp_ast::AstNode;
        let topology = a
            .topology_map
            .values()
            .find(|t| t.qualified_name == "Sys")
            .expect("topology Sys resolved");

        let group_names: Vec<&str> = topology
            .raw_direct_graphs
            .iter()
            .map(|g| g.name.data.as_str())
            .collect();
        assert_eq!(group_names, vec!["C"]);

        // Each group name must correspond to a connection_map entry (the diagram
        // target) and carry its own node id (its lens anchor).
        for graph in &topology.raw_direct_graphs {
            assert!(topology.connection_map.contains_key(&graph.name.data));
            // The group name node is distinct from the topology name node, so the
            // lens lands on the group, not the topology.
            assert_ne!(graph.name.id(), topology.symbol.name().id());
        }
    });
}

#[test]
fn connection_group_includes_only_participants() {
    with_analysis(MODEL, |a| {
        let diagram = fpp_diagram::lower(a, DiagramKind::ConnectionGroup, "Sys.C")
            .expect("connection group Sys.C should lower");
        assert_eq!(diagram.kind, DiagramKind::ConnectionGroup);
        assert_eq!(diagram.edges.len(), 1);
        let mut ids: Vec<&str> = diagram.nodes.iter().map(|n| n.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["cons", "prod"]);
    });
}

#[test]
fn lowers_to_sprotty_graph() {
    with_analysis(MODEL, |a| {
        let model = fpp_diagram::lower_to_smodel(
            a,
            DiagramKind::Topology,
            "Sys",
            false,
            fpp_diagram::TransitionActionMode::Uml,
        )
        .expect("topology Sys should lower to sprotty");

        assert_eq!(model["type"], "graph");
        assert_eq!(model["id"], "root");

        let children = model["children"].as_array().unwrap();
        // Two component nodes + one edge.
        let node_count = children
            .iter()
            .filter(|c| c["type"] == "node:component")
            .count();
        let edge_count = children.iter().filter(|c| c["type"] == "edge").count();
        assert_eq!(node_count, 2);
        assert_eq!(edge_count, 1);

        // A component node carries its kind and has port + label children.
        let prod = children
            .iter()
            .find(|c| c["id"] == "prod")
            .expect("prod node in sprotty model");
        assert_eq!(prod["kind"], "passive");
        let prod_children = prod["children"].as_array().unwrap();
        let port = prod_children
            .iter()
            .find(|c| c["type"] == "port" && c["id"] == "prod.pOut.0")
            .expect("pOut port");
        assert_eq!(port["isOutput"], true);
        assert_eq!(port["kind"], "output");

        // The edge references the exact port ids.
        let edge = children.iter().find(|c| c["type"] == "edge").unwrap();
        assert_eq!(edge["sourceId"], "prod.pOut.0");
        assert_eq!(edge["targetId"], "cons.cIn.0");
    });
}

#[test]
fn prune_unused_ports_keeps_only_connected() {
    with_analysis(MODEL, |a| {
        let mut diagram =
            fpp_diagram::lower(a, DiagramKind::Topology, "Sys").expect("topology Sys should lower");

        // Before pruning, Producer has an `unused` port and Consumer has `gIn`/`cOut`.
        let prod = diagram.nodes.iter().find(|n| n.id == "prod").unwrap();
        assert!(prod.ports.iter().any(|p| p.name == "unused"));

        diagram.prune_unused_ports();

        // After pruning, only the connected ports remain.
        let prod = diagram.nodes.iter().find(|n| n.id == "prod").unwrap();
        assert_eq!(
            prod.ports
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            vec!["pOut"]
        );
        let cons = diagram.nodes.iter().find(|n| n.id == "cons").unwrap();
        assert_eq!(
            cons.ports
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            vec!["cIn"]
        );
    });
}

#[test]
fn prune_is_noop_for_component_diagram() {
    with_analysis(MODEL, |a| {
        let mut diagram = fpp_diagram::lower(a, DiagramKind::Component, "Consumer")
            .expect("component Consumer should lower");
        let before = diagram.nodes[0].ports.len();
        diagram.prune_unused_ports();
        assert_eq!(diagram.nodes[0].ports.len(), before);
    });
}

#[test]
fn missing_element_is_an_error() {
    with_analysis(MODEL, |a| {
        let err = fpp_diagram::lower(a, DiagramKind::Topology, "DoesNotExist").unwrap_err();
        assert!(matches!(err, fpp_diagram::LowerError::NotFound { .. }));
    });
}
