use crate::{run_test, run_test_with};

#[test]
fn bad_channel() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/bad_channel")
}

#[test]
fn bad_omit_channel() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/bad_omit_channel")
}

#[test]
fn channel_neither_used_nor_omitted() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/channel_neither_used_nor_omitted",
    )
}

#[test]
fn channel_used_and_omitted() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/channel_used_and_omitted",
    )
}

#[test]
fn duplicate_id_explicit() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/duplicate_id_explicit",
    )
}

#[test]
fn duplicate_id_implicit() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/duplicate_id_implicit",
    )
}

#[test]
fn duplicate_packet_name() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/duplicate_packet_name",
    )
}

#[test]
fn duplicate_set_name() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/duplicate_set_name")
}

#[test]
fn id_not_numeric() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/id_not_numeric")
}

#[test]
fn instance_not_defined() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/instance_not_defined",
    )
}

#[test]
fn instance_not_in_topology() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/instance_not_in_topology",
    )
}

#[test]
fn level_not_numeric() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/level_not_numeric")
}

#[test]
fn level_out_of_range() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/level_out_of_range")
}

#[test]
fn negative_id() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/negative_id")
}

#[test]
fn negative_level() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/negative_level")
}

#[test]
fn not_deployment_topology() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/not_deployment_topology",
    )
}

#[test]
fn ok() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/ok")
}

#[test]
fn omit_instance_not_defined() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/omit_instance_not_defined",
    )
}

#[test]
fn omit_instance_not_in_topology() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/omit_instance_not_in_topology",
    )
}

#[test]
fn template_arg_channel() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/template_arg_channel",
    )
}

#[test]
fn template_arg_topology() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/template_arg_topology",
    )
}

#[test]
fn template_channel_neither_used_nor_omitted() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/template_channel_neither_used_nor_omitted",
    )
}

#[test]
fn template_ok() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/template_ok")
}

#[test]
fn shadowed_instance_error() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/shadowed_instance_error",
    )
}

#[test]
fn shadowed_instance_ok() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/shadowed_instance_ok",
    )
}

// Rust-only tests (no Scala counterpart)

/// Duplicate packet names are reported in model order, whatever their ids
#[test]
fn duplicate_packet_name_explicit_ids() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/duplicate_packet_name_explicit_ids",
    )
}

/// The instance of a packet channel must be a component instance
#[test]
fn channel_instance_not_component_instance() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/channel_instance_not_component_instance",
    )
}

/// Packets in a topology that is not a deployment, after an instance error
#[test]
fn not_deployment_after_instance_error() {
    run_test("tlm_packets/not_deployment_after_instance_error")
}

/// Packets in a topology that is not a deployment, after a connection error
#[test]
fn not_deployment_after_connection_error() {
    run_test("tlm_packets/not_deployment_after_connection_error")
}

/// Deployment topologies are visited in the order of the input files
#[test]
fn topology_order_files() {
    run_test_with(
        &["tlm_packets/instances", "tlm_packets/topology_order_b"],
        "tlm_packets/topology_order_a",
    )
}

/// A deployment topology from an expansion is visited at the expansion
#[test]
fn topology_order_template() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/topology_order_template",
    )
}

/// A deployment topology in an included file is visited at the include
#[test]
fn topology_order_include() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/topology_order_include",
    )
}

/// The channels of a packet set are checked in the order of the instance names
#[test]
fn channel_order_instances() {
    run_test("tlm_packets/channel_order_instances")
}

/// The channels of an instance are checked in declaration order
#[test]
fn channel_order_ids() {
    run_test("tlm_packets/channel_order_ids")
}

/// A channel in several packets is reported as used in the last one in model order
#[test]
fn used_channel_order() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/used_channel_order")
}
