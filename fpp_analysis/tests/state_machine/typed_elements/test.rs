use crate::run_test;

#[test]
fn choice_alias_alias() {
    run_test("state_machine/typed_elements/choice_alias_alias")
}

#[test]
fn choice_alias_i16_i32() {
    run_test("state_machine/typed_elements/choice_alias_i16_i32")
}

#[test]
fn choice_f32_f64() {
    run_test("state_machine/typed_elements/choice_f32_f64")
}

#[test]
fn choice_i16_i32() {
    run_test("state_machine/typed_elements/choice_i16_i32")
}

#[test]
fn choice_i32_f32() {
    run_test("state_machine/typed_elements/choice_i32_f32")
}

#[test]
fn choice_shadowed_error() {
    run_test("state_machine/typed_elements/choice_shadowed_error")
}

#[test]
fn choice_u32_bool() {
    run_test("state_machine/typed_elements/choice_u32_bool")
}

#[test]
fn choice_u32_bool_transitive() {
    run_test("state_machine/typed_elements/choice_u32_bool_transitive")
}

#[test]
fn choice_u32_none() {
    run_test("state_machine/typed_elements/choice_u32_none")
}

#[test]
fn sm_initial_bad_action_type() {
    run_test("state_machine/typed_elements/sm_initial_bad_action_type")
}

#[test]
fn sm_choice_bad_else_action_type() {
    run_test("state_machine/typed_elements/sm_choice_bad_else_action_type")
}

#[test]
fn sm_choice_bad_guard_type() {
    run_test("state_machine/typed_elements/sm_choice_bad_guard_type")
}

#[test]
fn sm_choice_bad_if_action_type() {
    run_test("state_machine/typed_elements/sm_choice_bad_if_action_type")
}

#[test]
fn state_choice_bad_else_action_type() {
    run_test("state_machine/typed_elements/state_choice_bad_else_action_type")
}

#[test]
fn state_choice_bad_guard_type() {
    run_test("state_machine/typed_elements/state_choice_bad_guard_type")
}

#[test]
fn state_entry_bad_action_type() {
    run_test("state_machine/typed_elements/state_entry_bad_action_type")
}

#[test]
fn state_exit_bad_action_type() {
    run_test("state_machine/typed_elements/state_exit_bad_action_type")
}

#[test]
fn state_external_transition_bad_action_type() {
    run_test("state_machine/typed_elements/state_external_transition_bad_action_type")
}

#[test]
fn state_initial_bad_action_type() {
    run_test("state_machine/typed_elements/state_initial_bad_action_type")
}

#[test]
fn state_choice_bad_if_action_type() {
    run_test("state_machine/typed_elements/state_choice_bad_if_action_type")
}

#[test]
fn state_choice_bad_if_action_type_f32_f64() {
    run_test("state_machine/typed_elements/state_choice_bad_if_action_type_f32_f64")
}

#[test]
fn state_choice_bad_if_action_type_i16_i32() {
    run_test("state_machine/typed_elements/state_choice_bad_if_action_type_i16_i32")
}

#[test]
fn state_self_transition_bad_action_type() {
    run_test("state_machine/typed_elements/state_self_transition_bad_action_type")
}

#[test]
fn state_transition_bad_guard_type() {
    run_test("state_machine/typed_elements/state_transition_bad_guard_type")
}
