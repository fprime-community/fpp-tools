use crate::run_test_with;

#[test]
fn not_deployment_topology() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/not_deployment_topology",
    )
}

#[test]
fn instance_not_defined() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/instance_not_defined",
    )
}

#[test]
fn id_not_numeric() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/id_not_numeric")
}

#[test]
fn channel_neither_used_nor_omitted() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/channel_neither_used_nor_omitted",
    )
}

#[test]
fn level_not_numeric() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/level_not_numeric")
}

#[test]
fn ok() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/ok")
}

#[test]
fn negative_id() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/negative_id")
}

#[test]
fn omit_instance_not_in_topology() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/omit_instance_not_in_topology",
    )
}

#[test]
fn bad_channel() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/bad_channel")
}

#[test]
fn omit_instance_not_defined() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/omit_instance_not_defined",
    )
}

#[test]
fn duplicate_set_name() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/duplicate_set_name")
}

#[test]
fn bad_omit_channel() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/bad_omit_channel")
}

#[test]
fn negative_level() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/negative_level")
}

#[test]
fn level_out_of_range() {
    run_test_with(&["tlm_packets/instances"], "tlm_packets/level_out_of_range")
}

#[test]
fn duplicate_id_implicit() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/duplicate_id_implicit",
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
fn duplicate_packet_name() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/duplicate_packet_name",
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
fn duplicate_id_explicit() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/duplicate_id_explicit",
    )
}

#[test]
fn channel_instance_not_component_instance() {
    run_test_with(
        &["tlm_packets/instances"],
        "tlm_packets/channel_instance_not_component_instance",
    )
}
