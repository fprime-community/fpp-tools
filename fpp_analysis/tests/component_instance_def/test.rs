use crate::run_test;

#[test]
fn active_no_priority() {
    run_test("component_instance_def/active_no_priority")
}

#[test]
fn active_no_queue_size() {
    run_test("component_instance_def/active_no_queue_size")
}

#[test]
fn active_no_stack_size() {
    run_test("component_instance_def/active_no_stack_size")
}

#[test]
fn conflicting_ids() {
    run_test("component_instance_def/conflicting_ids")
}

#[test]
fn conflicting_ids_empty_range_first() {
    run_test("component_instance_def/conflicting_ids_empty_range_first")
}

#[test]
fn conflicting_ids_empty_range_second() {
    run_test("component_instance_def/conflicting_ids_empty_range_second")
}

#[test]
fn conflicting_ids_record() {
    run_test("component_instance_def/conflicting_ids_record")
}

#[test]
fn invalid_negative_int() {
    run_test("component_instance_def/invalid_negative_int")
}

#[test]
fn large_int() {
    run_test("component_instance_def/large_int")
}

#[test]
fn ok() {
    run_test("component_instance_def/ok")
}

#[test]
fn passive_cpu() {
    run_test("component_instance_def/passive_cpu")
}

#[test]
fn passive_priority() {
    run_test("component_instance_def/passive_priority")
}

#[test]
fn passive_queue_size() {
    run_test("component_instance_def/passive_queue_size")
}

#[test]
fn passive_stack_size() {
    run_test("component_instance_def/passive_stack_size")
}

#[test]
fn queued_cpu() {
    run_test("component_instance_def/queued_cpu")
}

#[test]
fn queued_no_queue_size() {
    run_test("component_instance_def/queued_no_queue_size")
}

#[test]
fn queued_priority() {
    run_test("component_instance_def/queued_priority")
}

#[test]
fn queued_stack_size() {
    run_test("component_instance_def/queued_stack_size")
}

#[test]
fn shadowed_error() {
    run_test("component_instance_def/shadowed_error")
}

#[test]
fn shadowed_ok() {
    run_test("component_instance_def/shadowed_ok")
}

#[test]
fn two_empty_ranges() {
    run_test("component_instance_def/two_empty_ranges")
}

#[test]
fn undef_component() {
    run_test("component_instance_def/undef_component")
}

// Rust-only tests (no Scala counterpart)

/// The base identifier of a component instance must have a numeric type
#[test]
fn base_id_not_numeric() {
    run_test("component_instance_def/base_id_not_numeric")
}

/// The queue size of a component instance must have a numeric type
#[test]
fn queue_size_not_numeric() {
    run_test("component_instance_def/queue_size_not_numeric")
}

/// The CPU affinity of a component instance must have a numeric type
#[test]
fn cpu_not_numeric() {
    run_test("component_instance_def/cpu_not_numeric")
}

/// A passive component instance may not have a queue size, whatever its type
#[test]
fn passive_queue_size_not_numeric() {
    run_test("component_instance_def/passive_queue_size_not_numeric")
}

/// The expressions of a component instance are checked after the component definitions
#[test]
fn not_numeric_after_component_error() {
    run_test("component_instance_def/not_numeric_after_component_error")
}

/// Overlapping ID ranges of instances with equal base IDs are reported in definition order
#[test]
fn conflicting_ids_equal_base() {
    run_test("component_instance_def/conflicting_ids_equal_base")
}

/// An ID range past the 128-bit integer range is an error
#[test]
fn base_id_overflow() {
    run_test("component_instance_def/base_id_overflow")
}
