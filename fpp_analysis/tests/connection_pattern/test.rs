use crate::run_test;

#[test]
fn event_missing_target_port() {
    run_test("connection_pattern/event_missing_target_port")
}

#[test]
fn time_missing_source_port() {
    run_test("connection_pattern/time_missing_source_port")
}

#[test]
fn telemetry_two_source_ports() {
    run_test("connection_pattern/telemetry_two_source_ports")
}

#[test]
fn health_ok() {
    run_test("connection_pattern/health_ok")
}

#[test]
fn telemetry_missing_source_port() {
    run_test("connection_pattern/telemetry_missing_source_port")
}

#[test]
fn undef_target() {
    run_test("connection_pattern/undef_target")
}

#[test]
fn command_missing_source_port() {
    run_test("connection_pattern/command_missing_source_port")
}

#[test]
fn event_two_source_ports() {
    run_test("connection_pattern/event_two_source_ports")
}

#[test]
fn text_event_two_source_ports() {
    run_test("connection_pattern/text_event_two_source_ports")
}

#[test]
fn text_event_missing_source_port() {
    run_test("connection_pattern/text_event_missing_source_port")
}

#[test]
fn event_ok() {
    run_test("connection_pattern/event_ok")
}

#[test]
fn param_missing_target_port() {
    run_test("connection_pattern/param_missing_target_port")
}

#[test]
fn undef_source() {
    run_test("connection_pattern/undef_source")
}

#[test]
fn command_ok() {
    run_test("connection_pattern/command_ok")
}

#[test]
fn health_missing_port() {
    run_test("connection_pattern/health_missing_port")
}

#[test]
fn telemetry_ok() {
    run_test("connection_pattern/telemetry_ok")
}

#[test]
fn duplicate_pattern() {
    run_test("connection_pattern/duplicate_pattern")
}

#[test]
fn command_two_source_ports() {
    run_test("connection_pattern/command_two_source_ports")
}

#[test]
fn time_two_source_ports() {
    run_test("connection_pattern/time_two_source_ports")
}

#[test]
fn time_missing_target_port() {
    run_test("connection_pattern/time_missing_target_port")
}

#[test]
fn param_two_source_ports() {
    run_test("connection_pattern/param_two_source_ports")
}

#[test]
fn event_missing_source_port() {
    run_test("connection_pattern/event_missing_source_port")
}

#[test]
fn health_duplicate_port() {
    run_test("connection_pattern/health_duplicate_port")
}

#[test]
fn param_ok() {
    run_test("connection_pattern/param_ok")
}

#[test]
fn text_event_missing_target_port() {
    run_test("connection_pattern/text_event_missing_target_port")
}

#[test]
fn param_missing_source_port() {
    run_test("connection_pattern/param_missing_source_port")
}

#[test]
fn text_event_ok() {
    run_test("connection_pattern/text_event_ok")
}

#[test]
fn command_missing_target_port() {
    run_test("connection_pattern/command_missing_target_port")
}

#[test]
fn telemetry_missing_target_port() {
    run_test("connection_pattern/telemetry_missing_target_port")
}

#[test]
fn time_ok() {
    run_test("connection_pattern/time_ok")
}

#[test]
fn source_not_component_instance() {
    run_test("connection_pattern/source_not_component_instance")
}

#[test]
fn target_not_component_instance() {
    run_test("connection_pattern/target_not_component_instance")
}
