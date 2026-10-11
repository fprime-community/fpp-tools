use crate::run_test;

#[test]
fn async_input_active() {
    run_test("port_instance/async_input_active")
}

#[test]
fn async_input_passive() {
    run_test("port_instance/async_input_passive")
}

#[test]
fn async_input_ref_params() {
    run_test("port_instance/async_input_ref_params")
}

#[test]
fn async_input_return_value() {
    run_test("port_instance/async_input_return_value")
}

#[test]
fn async_product_recv_active() {
    run_test("port_instance/async_product_recv_active")
}

#[test]
fn async_product_recv_passive() {
    run_test("port_instance/async_product_recv_passive")
}

#[test]
fn bad_array_size() {
    run_test("port_instance/bad_array_size")
}

#[test]
fn bad_priority() {
    run_test("port_instance/bad_priority")
}

#[test]
fn bad_priority_product_recv() {
    run_test("port_instance/bad_priority_product_recv")
}

#[test]
fn duplicate_command_recv() {
    run_test("port_instance/duplicate_command_recv")
}

#[test]
fn duplicate_general() {
    run_test("port_instance/duplicate_general")
}

#[test]
fn ok() {
    run_test("port_instance/ok")
}

#[test]
fn shadowed_time_get() {
    run_test("port_instance/shadowed_time_get")
}

#[test]
fn special_input_kind_command() {
    run_test("port_instance/special_input_kind_command")
}

#[test]
fn special_input_kind_missing_product_recv() {
    run_test("port_instance/special_input_kind_missing_product_recv")
}

#[test]
fn sync_input_priority() {
    run_test("port_instance/sync_input_priority")
}

#[test]
fn sync_input_queue_full() {
    run_test("port_instance/sync_input_queue_full")
}

#[test]
fn sync_product_recv_priority() {
    run_test("port_instance/sync_product_recv_priority")
}

#[test]
fn sync_product_recv_queue_full() {
    run_test("port_instance/sync_product_recv_queue_full")
}

#[test]
fn undef_command_recv() {
    run_test("port_instance/undef_command_recv")
}

#[test]
fn undef_command_reg() {
    run_test("port_instance/undef_command_reg")
}

#[test]
fn undef_command_resp() {
    run_test("port_instance/undef_command_resp")
}

#[test]
fn undef_event() {
    run_test("port_instance/undef_event")
}

#[test]
fn undef_general() {
    run_test("port_instance/undef_general")
}

#[test]
fn undef_param_get() {
    run_test("port_instance/undef_param_get")
}

#[test]
fn undef_param_set() {
    run_test("port_instance/undef_param_set")
}

#[test]
fn undef_product_recv() {
    run_test("port_instance/undef_product_recv")
}

#[test]
fn undef_product_request() {
    run_test("port_instance/undef_product_request")
}

#[test]
fn undef_product_send() {
    run_test("port_instance/undef_product_send")
}

#[test]
fn undef_telemetry() {
    run_test("port_instance/undef_telemetry")
}

#[test]
fn undef_text_event() {
    run_test("port_instance/undef_text_event")
}

#[test]
fn undef_time_get() {
    run_test("port_instance/undef_time_get")
}

// Rust-only tests (no Scala counterpart)

/// An output port may not have queue full behavior
#[test]
fn output_queue_full() {
    run_test("port_instance/output_queue_full")
}

/// The implied use of a special port that names a module is an error
#[test]
fn special_port_module() {
    run_test("port_instance/special_port_module")
}

/// An undefined array size is reported before an undefined port type
#[test]
fn undef_size_and_port() {
    run_test("port_instance/undef_size_and_port")
}

/// An undefined priority is reported before an undefined implied port
#[test]
fn undef_priority_and_implied_port() {
    run_test("port_instance/undef_priority_and_implied_port")
}

/// The qualifier of an implied port use that is not a qualifier
#[test]
fn implied_port_qualifier_not_qualifier() {
    run_test("port_instance/implied_port_qualifier_not_qualifier")
}

/// An implied port use whose qualifier lacks the port
#[test]
fn implied_port_undef_member() {
    run_test("port_instance/implied_port_undef_member")
}

/// A port array size outside the 32-bit range is an error
#[test]
fn array_size_out_of_range() {
    run_test("port_instance/array_size_out_of_range")
}
