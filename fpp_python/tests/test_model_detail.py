"""Detailed navigation of the semantic model reached through `model.analysis`:
component sub-element maps (commands / events / params / telemetry), the
`NonParamKind` union subclasses, and the state-machine model.
"""

import fpp as f
from fpp import (
    AsyncNonParamKind,
    Command,
    GuardedNonParamKind,
    Kind,
    Loc,
    NonParamCommand,
    NonParamKind,
    QueueFull,
    StateMachineSymbol,
    StateMachineSymbolAction,
    StateMachineSymbolSignal,
    SymbolStateMachine,
    SyncNonParamKind,
)


def _model(path: str):
    m = f.analyze(path)
    assert not m.has_errors, [d.message for d in m.diagnostics]
    return m


def _only_component(m):
    (comp,) = m.analysis.component_map.values()
    return comp


def test_commands_from_fixture():
    m = _model("tests/commands/commands.fpp")
    comp = _only_component(m)
    assert m.analysis.get_qualified_name(comp.symbol) == "PriorityQueueFull"
    # `command_map` is keyed by opcode; the fixture assigns 0, 1 and (explicit) 0x10.
    cmds = comp.command_map
    assert set(cmds) == {0, 1, 0x10}
    assert cmds[0].name == "COMMAND_1"
    assert cmds[0x10].name == "COMMAND_3"
    # Every command in this fixture is a non-param async command with a priority.
    for cmd in cmds.values():
        assert cmd.is_async
        assert isinstance(cmd, NonParamCommand)
        assert isinstance(cmd, Command)
        assert isinstance(cmd.kind, AsyncNonParamKind)
        assert isinstance(cmd.kind, NonParamKind)
        assert isinstance(cmd.node.location, Loc)
    # Only COMMAND_3 declares its opcode; 0 and 1 are auto-assigned.
    assert cmds[0].node.opcode is None
    assert cmds[0x10].node.opcode is not None
    assert cmds[0].kind.priority == 10
    assert cmds[1].kind.priority == 20
    # COMMAND_3 declares `drop`; the others default to the assert behavior.
    assert cmds[0x10].kind.queue_full == QueueFull.Drop
    assert cmds[0].kind.queue_full == QueueFull.Assert


def test_non_param_kinds_are_union_subclasses():
    # A component with one of each command kind exercises all three subclasses.
    src = """
    module Fw {
      port Cmd
      port CmdReg
      port CmdResponse
    }
    active component K {
      command recv port cmdIn
      command reg port cmdRegOut
      command resp port cmdResponseOut
      sync command S_CMD
      guarded command G_CMD
      async command A_CMD
    }
    """
    m = f.analyze(source=src, uri="k.fpp")
    assert not m.has_errors, [d.message for d in m.diagnostics]
    comp = _only_component(m)
    by_name = {c.name: c.kind for c in comp.command_map.values()}
    assert isinstance(by_name["S_CMD"], SyncNonParamKind)
    assert isinstance(by_name["G_CMD"], GuardedNonParamKind)
    assert isinstance(by_name["A_CMD"], AsyncNonParamKind)
    assert all(isinstance(k, NonParamKind) for k in by_name.values())


def test_events_from_fixture():
    m = _model("tests/events/events.fpp")
    a = m.analysis
    comps = {a.get_qualified_name(s): c for s, c in a.component_map.items()}
    assert {"EventIdentifiers", "M.EventThrottling"} <= set(comps)
    events = comps["EventIdentifiers"].event_map
    # Two explicit ids (0x10, 0x11) plus one auto-assigned (0x12); the id is the key.
    assert set(events) == {0x10, 0x11, 0x12}
    assert events[0x10].name == "Event1"
    assert {e.name for e in events.values()} == {"Event1", "Event2", "Event3"}
    assert all(isinstance(e.node.location, Loc) for e in events.values())


def test_params_from_fixture():
    m = _model("tests/parameters/parameters.fpp")
    comp = _only_component(m)
    params = comp.param_map
    by_name = {p.name: p for p in params.values()}
    assert set(by_name) == {"Param1", "Param2", "Param3", "Param4"}
    # Param1 gets implied set/save opcodes 0x00 / 0x01.
    assert by_name["Param1"].set_opcode == 0
    assert by_name["Param1"].save_opcode == 1
    # Param2 declares explicit set/save opcodes.
    assert by_name["Param2"].set_opcode == 0x10
    assert by_name["Param2"].save_opcode == 0x11
    # Only Param4 is external.
    assert by_name["Param4"].is_external is True
    assert by_name["Param1"].is_external is False


def test_telemetry_and_dicts():
    src = """
    module Fw {
      port Cmd
      port CmdReg
      port CmdResponse
      port Tlm
      port Time
    }
    passive component K {
      command recv port cmdIn
      command reg port cmdRegOut
      command resp port cmdResponseOut
      telemetry port tlmOut
      time get port timeGetOut
      telemetry CH1: U32
      telemetry CH2: F32 id 0x10
    }
    """
    m = f.analyze(source=src, uri="tlm.fpp")
    assert not m.has_errors, [d.message for d in m.diagnostics]
    comp = _only_component(m)
    # Telemetry channels are defined; no `command` definitions (only cmd ports).
    assert comp.has_telemetry is True
    assert comp.has_commands is False
    # `tlm_channel_map` is keyed by channel id: CH1 auto-assigns 0, CH2 declares 0x10.
    channels = {i: t.name for i, t in comp.tlm_channel_map.items()}
    assert channels == {0: "CH1", 0x10: "CH2"}
    # The name-keyed mirror agrees.
    assert set(comp.tlm_channel_name_map) == {"CH1", "CH2"}
    assert comp.tlm_channel_name_map["CH2"].name == "CH2"
    # Walking the id keys in order gives declaration order here.
    assert [comp.tlm_channel_map[i].name for i in sorted(comp.tlm_channel_map)] == [
        "CH1",
        "CH2",
    ]


SM_SRC = """
module M {
  state machine SM {
    action a
    guard g
    signal s
    initial enter S1
    state S1 {
      on s enter S2
    }
    state S2 {
      entry do { a }
      initial enter S3
      state S3
    }
  }
}
"""


def test_state_machine_model():
    m = f.analyze(source=SM_SRC, uri="sm.fpp")
    assert not m.has_errors, [d.message for d in m.diagnostics]
    a = m.analysis
    (sym, sm) = next(iter(a.state_machine_map.items()))
    assert isinstance(sym, SymbolStateMachine)
    assert a.get_qualified_name(sym) == "M.SM"
    assert sm.node.name == "SM"
    assert sm.kind == Kind.Internal
    assert sm.blocking_error is False
    assert sm.has_actions and sm.has_guards and sm.has_signals

    (action,) = sm.actions
    assert isinstance(action, StateMachineSymbolAction)
    assert isinstance(action, StateMachineSymbol)
    assert action.unqualified_name == "a"
    assert action.definition.name == "a"

    (signal,) = sm.signals
    assert isinstance(signal, StateMachineSymbolSignal)
    assert signal.unqualified_name == "s"
