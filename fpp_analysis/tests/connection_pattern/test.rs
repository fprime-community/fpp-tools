use crate::run_test;

#[test]
fn command_missing_source_port() {
    run_test("connection_pattern/command_missing_source_port")
}

#[test]
fn command_missing_target_port() {
    run_test("connection_pattern/command_missing_target_port")
}

#[test]
fn command_ok() {
    run_test("connection_pattern/command_ok")
}

#[test]
fn command_two_source_ports() {
    run_test("connection_pattern/command_two_source_ports")
}

#[test]
fn duplicate_pattern() {
    run_test("connection_pattern/duplicate_pattern")
}

#[test]
fn event_missing_source_port() {
    run_test("connection_pattern/event_missing_source_port")
}

#[test]
fn event_missing_target_port() {
    run_test("connection_pattern/event_missing_target_port")
}

#[test]
fn event_ok() {
    run_test("connection_pattern/event_ok")
}

#[test]
fn event_two_source_ports() {
    run_test("connection_pattern/event_two_source_ports")
}

#[test]
fn health_duplicate_port() {
    run_test("connection_pattern/health_duplicate_port")
}

#[test]
fn health_missing_port() {
    run_test("connection_pattern/health_missing_port")
}

#[test]
fn health_ok() {
    run_test("connection_pattern/health_ok")
}

#[test]
fn param_missing_source_port() {
    run_test("connection_pattern/param_missing_source_port")
}

#[test]
fn param_missing_target_port() {
    run_test("connection_pattern/param_missing_target_port")
}

#[test]
fn param_ok() {
    run_test("connection_pattern/param_ok")
}

#[test]
fn param_two_source_ports() {
    run_test("connection_pattern/param_two_source_ports")
}

#[test]
fn telemetry_missing_source_port() {
    run_test("connection_pattern/telemetry_missing_source_port")
}

#[test]
fn telemetry_missing_target_port() {
    run_test("connection_pattern/telemetry_missing_target_port")
}

#[test]
fn telemetry_ok() {
    run_test("connection_pattern/telemetry_ok")
}

#[test]
fn telemetry_two_source_ports() {
    run_test("connection_pattern/telemetry_two_source_ports")
}

#[test]
fn text_event_missing_source_port() {
    run_test("connection_pattern/text_event_missing_source_port")
}

#[test]
fn text_event_missing_target_port() {
    run_test("connection_pattern/text_event_missing_target_port")
}

#[test]
fn text_event_ok() {
    run_test("connection_pattern/text_event_ok")
}

#[test]
fn text_event_two_source_ports() {
    run_test("connection_pattern/text_event_two_source_ports")
}

#[test]
fn time_missing_source_port() {
    run_test("connection_pattern/time_missing_source_port")
}

#[test]
fn time_missing_target_port() {
    run_test("connection_pattern/time_missing_target_port")
}

#[test]
fn time_ok() {
    run_test("connection_pattern/time_ok")
}

#[test]
fn time_two_source_ports() {
    run_test("connection_pattern/time_two_source_ports")
}

#[test]
fn undef_source() {
    run_test("connection_pattern/undef_source")
}

#[test]
fn undef_target() {
    run_test("connection_pattern/undef_target")
}

// Rust-only tests (no Scala counterpart)

/// The ports of an ambiguous pattern are listed in the order the component declares them
#[test]
fn time_two_source_ports_order() {
    run_test("connection_pattern/time_two_source_ports_order")
}

/// The source of a connection pattern must be a component instance
#[test]
fn source_not_component_instance() {
    run_test("connection_pattern/source_not_component_instance")
}

/// A target of a connection pattern must be a component instance
#[test]
fn target_not_component_instance() {
    run_test("connection_pattern/target_not_component_instance")
}

/// An undefined pattern source is reported before an undefined target
#[test]
fn undef_source_and_target() {
    run_test("connection_pattern/undef_source_and_target")
}

/// Pattern errors are reported before direct connection errors
#[test]
fn pattern_before_direct() {
    run_test("connection_pattern/pattern_before_direct")
}

/// Connection patterns are checked in definition order
#[test]
fn pattern_order() {
    run_test("connection_pattern/pattern_order")
}
