use crate::run_test;

#[test]
fn action() {
    run_test("state_machine/redef/action")
}

#[test]
fn choice() {
    run_test("state_machine/redef/choice")
}

#[test]
fn constant() {
    run_test("state_machine/redef/constant")
}

#[test]
fn guard() {
    run_test("state_machine/redef/guard")
}

#[test]
fn nested_choice() {
    run_test("state_machine/redef/nested_choice")
}

#[test]
fn nested_state() {
    run_test("state_machine/redef/nested_state")
}

#[test]
fn ok() {
    run_test("state_machine/redef/ok")
}

#[test]
fn signal() {
    run_test("state_machine/redef/signal")
}

#[test]
fn state() {
    run_test("state_machine/redef/state")
}

#[test]
fn state_choice() {
    run_test("state_machine/redef/state_choice")
}

#[test]
fn state_enum() {
    run_test("state_machine/redef/state_enum")
}

#[test]
fn state_shadow_ok() {
    run_test("state_machine/redef/state_shadow_ok")
}

#[test]
fn r#type() {
    run_test("state_machine/redef/type")
}
