//! End-to-end regression tests for URI keying across the workspace pipeline.
//!
//! The editor keys every document by the exact URI it opened. The server must
//! key its VFS and analysis caches under those same URIs, or cross-file symbol
//! resolution (hover, goto, semantic highlighting of symbol *uses*) silently
//! fails: the lookup misses, no `SourceFile` is found, and only the purely
//! syntactic tokens survive.
//!
//! These tests open a project through a *symlinked* root — the path the editor
//! hands us is not the canonical one — and assert the opened file is still
//! reachable and its uses are analyzed. This is the scenario that regressed
//! when include resolution canonicalized paths (resolving the symlink) before
//! turning them back into URIs.
#![cfg(test)]

use crate::global_state::{GlobalState, Task};
use crate::lsp::capabilities::ClientCapabilities;
use fpp_analysis::semantics::{NameGroup, SymbolInterface};
use lsp_types::{Uri, WorkspaceFolder};
use std::path::Path;
use std::str::FromStr;

/// Fresh, empty directory under `target/` for a test's fixture files.
fn fixture_dir(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Index a workspace rooted at `folder` and run analysis to completion.
fn index_workspace(folder: &Path) -> GlobalState {
    let folder_uri = crate::uri::from_file_path(folder).unwrap();
    let workspace_folders = vec![WorkspaceFolder {
        uri: Uri::from_str(&folder_uri).unwrap(),
        name: "test".into(),
    }];

    let (tx, rx) = crossbeam_channel::unbounded();
    // Keep the receiver alive for the lifetime of the process so outgoing
    // notifications (e.g. progress) sent after this helper returns don't panic
    // on a closed channel.
    Box::leak(Box::new(rx));
    let mut state = GlobalState::new(
        Some(workspace_folders),
        tx,
        ClientCapabilities::new(Default::default()),
    );
    state.on_task(Task::ReloadWorkspace);
    state.run_pending_tasks();
    // Analysis is debounced behind a timer in the real loop; run it directly.
    state.on_task(Task::Analysis);
    state.run_pending_tasks();
    state
}

#[test]
fn scan_mode_resolves_uses_through_symlinked_root() {
    let real = fixture_dir("ws_scan_real");
    let link = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("ws_scan_link");
    let _ = std::fs::remove_file(&link);

    // `Top.fpp` includes `Defs.fppi` and uses the constant it defines.
    std::fs::write(real.join("Defs.fppi"), "constant A = 1\n").unwrap();
    std::fs::write(
        real.join("Top.fpp"),
        "module M {\n  include \"Defs.fppi\"\n  constant B = A\n}\n",
    )
    .unwrap();
    std::fs::write(real.join(".fpp-lsp"), "scanWorkspace: true\n").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    // The editor opened the project through the symlink.
    let state = index_workspace(&link);

    let top_uri = crate::uri::from_file_path(link.join("Top.fpp")).unwrap();
    assert!(
        state.source_file_for_uri(&top_uri).is_some(),
        "Top.fpp not reachable under the URI the editor opened ({top_uri})"
    );
    assert_eq!(
        state.use_def_count(),
        1,
        "the use of `A` in `constant B = A` was not resolved"
    );
}

/// A completion request can race ahead of analysis. Analysis is debounced, so
/// the compiler context can be rebuilt (dropping the nodes of a
/// no-longer-present translation unit) before the coalesced `Task::Analysis`
/// refreshes the snapshot. In that window `state.analysis` still holds
/// `Symbol`s pointing at node handles that are absent from the current context.
/// Building a completion item for such a stale symbol used to `unwrap()` the
/// missing node and panic; it must now degrade gracefully instead.
#[test]
fn completion_item_survives_stale_symbol_after_context_rebuild() {
    let dir = fixture_dir("ws_stale_symbol");

    // A pre-annotated constant. The annotation forces `symbol_to_completion_item`
    // down the branch that reads the symbol's backing node from the context.
    std::fs::write(
        dir.join("Top.fpp"),
        "module M {\n  @ doc\n  constant A = 1\n}\n",
    )
    .unwrap();
    std::fs::write(dir.join(".fpp-lsp"), "scanWorkspace: true\n").unwrap();

    let mut state = index_workspace(&dir);

    // Capture a symbol from the fresh analysis snapshot. Its node currently
    // resolves against the context.
    let symbol = state
        .snapshot_analysis()
        .global_scope
        .get(NameGroup::Value, "M")
        .and_then(|m| state.snapshot_analysis().symbol_scope_map.get(&m).cloned())
        .and_then(|scope| scope.get_group(NameGroup::Value).get("A"))
        .expect("constant `A` should be in module `M`'s scope");
    assert!(
        state.context.node_try_get(&symbol.node()).is_some(),
        "captured symbol's node should resolve before the rebuild"
    );

    // Delete the only source file and reindex. `Task::LoadFullWorkspace` builds a
    // fresh `CompilerContext` from the (now empty) file set, so the node handle
    // the captured symbol points at no longer exists. Run every task *except* the
    // debounced analysis so `state.analysis` remains the stale pre-rebuild
    // snapshot — exactly the racing-completion window.
    std::fs::remove_file(dir.join("Top.fpp")).unwrap();
    state.on_task(Task::LoadFullWorkspace);
    state.run_pending_tasks_except_analysis();
    assert!(
        state.context.node_try_get(&symbol.node()).is_none(),
        "captured symbol's node should have been dropped by the context rebuild"
    );

    // Building a completion item for the stale symbol must not panic.
    let item = crate::util::symbol_to_completion_item(&state, &symbol);
    assert_eq!(item.label, symbol.name().data);
}

/// Build a `CompletionParams` for `uri` at a zero-based `line`/`character`.
fn completion_params_at(uri: &str, line: u32, character: u32) -> lsp_types::CompletionParams {
    lsp_types::CompletionParams {
        text_document_position: lsp_types::TextDocumentPositionParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: Uri::from_str(uri).unwrap(),
            },
            position: lsp_types::Position { line, character },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    }
}

/// Flatten a completion response into its item list (empty when `None`).
fn completion_items(resp: Option<lsp_types::CompletionResponse>) -> Vec<lsp_types::CompletionItem> {
    match resp {
        None => vec![],
        Some(lsp_types::CompletionResponse::Array(items)) => items,
        Some(lsp_types::CompletionResponse::List(list)) => list.items,
    }
}

/// Completion must not fire while the cursor is inside a `#` comment. Comments
/// are trivia the completion resolver skips over, so without an explicit guard
/// a cursor parked in a trailing comment resolves against the preceding code
/// token (here the dangling `M.`) and wrongly offers that scope's members.
#[test]
fn completion_suppressed_inside_comment() {
    let dir = fixture_dir("ws_comment_completion");

    // `type T = M.` leaves a dangling member access; the trailing `# comment`
    // is where the cursor sits. Line/column are zero-based: the `#` is at
    // column 14 of line 1, so column 16 is inside the comment text.
    let text = "module M {\n  type T = M. # comment\n}\n";
    std::fs::write(dir.join("Top.fpp"), text).unwrap();
    std::fs::write(dir.join(".fpp-lsp"), "scanWorkspace: true\n").unwrap();

    let mut state = index_workspace(&dir);

    let top_uri = crate::uri::from_file_path(dir.join("Top.fpp")).unwrap();
    // Open the document in the VFS so the completion handler can read it.
    state.vfs.did_open(lsp_types::DidOpenTextDocumentParams {
        text_document: lsp_types::TextDocumentItem {
            uri: Uri::from_str(&top_uri).unwrap(),
            language_id: "fpp".into(),
            version: 1,
            text: text.to_string(),
        },
    });

    // Sanity check: completion at the dangling dot (before the comment) *does*
    // resolve `M`'s members, so this fixture exercises the resolving path.
    let at_dot = completion_items(
        crate::handlers::handle_completion(&state, completion_params_at(&top_uri, 1, 13))
            .expect("completion at dot should not error"),
    );
    assert!(
        at_dot.iter().any(|i| i.label == "T"),
        "fixture precondition: dangling `M.` should offer member `T`, got {at_dot:?}"
    );

    // Cursor inside the trailing comment must yield no completions.
    let in_comment = completion_items(
        crate::handlers::handle_completion(&state, completion_params_at(&top_uri, 1, 16))
            .expect("completion inside comment should not error"),
    );
    assert!(
        in_comment.is_empty(),
        "expected no completions inside a comment, got {in_comment:?}"
    );
}

#[test]
fn locs_mode_resolves_uses_through_symlinked_root() {
    let real = fixture_dir("ws_locs_real");
    let link = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("ws_locs_link");
    let _ = std::fs::remove_file(&link);

    std::fs::write(real.join("Defs.fppi"), "constant A = 1\n").unwrap();
    std::fs::write(
        real.join("Top.fpp"),
        "module M {\n  include \"Defs.fppi\"\n  constant B = A\n}\n",
    )
    .unwrap();
    std::fs::write(
        real.join("locs.fpp"),
        "locate constant M.B at \"Top.fpp\"\nlocate constant M.A at \"Defs.fppi\"\n",
    )
    .unwrap();
    std::fs::write(real.join(".fpp-lsp"), "locs: locs.fpp\n").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let state = index_workspace(&link);

    let top_uri = crate::uri::from_file_path(link.join("Top.fpp")).unwrap();
    assert!(
        state.source_file_for_uri(&top_uri).is_some(),
        "Top.fpp not reachable under the URI the editor opened ({top_uri})"
    );
    assert_eq!(
        state.use_def_count(),
        1,
        "the use of `A` in `constant B = A` was not resolved"
    );
}

/// The definitions a deployment topology needs before it can have a telemetry
/// dictionary: two instances of a component with one channel each.
const TLM_PACKET_DEFS: &str = "\
module Fw {
  port Time
  port Tlm
}

passive component C {
  time get port timeGetOut
  telemetry port tlmOut
  telemetry T: U32
}

instance c1: C base id 0x100
instance c2: C base id 0x200

";

/// Index `top` as a deployment topology, open it in the VFS, and return the
/// quick fixes offered at zero-based `line`/`character`.
fn code_actions_at(
    name: &str,
    top: &str,
    line: u32,
    character: u32,
) -> (String, Vec<lsp_types::CodeActionOrCommand>) {
    let dir = fixture_dir(name);
    let text = format!("{TLM_PACKET_DEFS}{top}");
    std::fs::write(dir.join("Top.fpp"), &text).unwrap();
    std::fs::write(dir.join(".fpp-lsp"), "scanWorkspace: true\n").unwrap();

    let mut state = index_workspace(&dir);
    let top_uri = crate::uri::from_file_path(dir.join("Top.fpp")).unwrap();
    let uri = Uri::from_str(&top_uri).unwrap();
    state.vfs.did_open(lsp_types::DidOpenTextDocumentParams {
        text_document: lsp_types::TextDocumentItem {
            uri: uri.clone(),
            language_id: "fpp".into(),
            version: 1,
            text: text.clone(),
        },
    });

    // The definitions are prepended, so callers give positions relative to `top`.
    let line = line + TLM_PACKET_DEFS.lines().count() as u32;
    let position = lsp_types::Position { line, character };
    let params = lsp_types::CodeActionParams {
        text_document: lsp_types::TextDocumentIdentifier { uri },
        range: lsp_types::Range {
            start: position,
            end: position,
        },
        context: Default::default(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let actions = fpp_core::run_ref(&state.context, || {
        crate::handlers::handle_code_action(&state, params)
    })
    .expect("code action request should not error")
    .unwrap_or_default();
    (text, actions)
}

/// Apply the single edit of the single quick fix to `text`.
fn apply_only_fix(text: &str, actions: &[lsp_types::CodeActionOrCommand]) -> String {
    let [lsp_types::CodeActionOrCommand::CodeAction(action)] = actions else {
        panic!("expected exactly one quick fix, got {actions:?}");
    };
    assert_eq!(action.kind, Some(lsp_types::CodeActionKind::QUICKFIX));
    let edits: Vec<lsp_types::TextEdit> = action
        .edit
        .clone()
        .and_then(|edit| edit.changes)
        .expect("quick fix should carry a workspace edit")
        .into_values()
        .flatten()
        .collect();
    let [edit] = &edits[..] else {
        panic!("quick fix should be a single text edit, got {edits:?}");
    };

    let lines = fpp_core::LineIndex::new(text);
    let offset = |position: &lsp_types::Position| {
        usize::from(
            lines
                .offset(fpp_core::LineCol {
                    line: position.line,
                    col: position.character,
                })
                .unwrap(),
        )
    };
    let mut out = text.to_string();
    out.replace_range(
        offset(&edit.range.start)..offset(&edit.range.end),
        &edit.new_text,
    );
    out
}

#[test]
fn quick_fix_opens_an_omit_block_for_uncovered_channels() {
    let top = "\
deployment topology T {
  instance c1
  instance c2
  telemetry packets P {
    packet P group 0 {
      c1.T
    }
  }
}
";
    // Cursor on the `telemetry packets P` line.
    let (text, actions) = code_actions_at("ws_omit_new_block", top, 3, 4);
    let [lsp_types::CodeActionOrCommand::CodeAction(action)] = &actions[..] else {
        panic!("expected exactly one quick fix, got {actions:?}");
    };
    assert_eq!(action.title, "Omit 1 telemetry channel");
    assert!(
        apply_only_fix(&text, &actions).ends_with(
            "\
  telemetry packets P {
    packet P group 0 {
      c1.T
    }
  } omit {
    c2.T
  }
}
"
        ),
        "unexpected fix result:\n{}",
        apply_only_fix(&text, &actions)
    );
}

#[test]
fn quick_fix_appends_to_an_existing_omit_block() {
    let top = "\
deployment topology T {
  instance c1
  instance c2
  telemetry packets P {
    packet P group 0 {
    }
  } omit {
    c1.T
  }
}
";
    let (text, actions) = code_actions_at("ws_omit_append", top, 3, 4);
    assert!(
        apply_only_fix(&text, &actions).ends_with(
            "\
  } omit {
    c1.T
    c2.T
  }
}
"
        ),
        "unexpected fix result:\n{}",
        apply_only_fix(&text, &actions)
    );
}

#[test]
fn quick_fix_fills_in_an_empty_omit_block() {
    let top = "\
deployment topology T {
  instance c1
  instance c2
  telemetry packets P {
    packet P group 0 {
    }
  } omit {}
}
";
    let (text, actions) = code_actions_at("ws_omit_empty_block", top, 3, 4);
    let [lsp_types::CodeActionOrCommand::CodeAction(action)] = &actions[..] else {
        panic!("expected exactly one quick fix, got {actions:?}");
    };
    assert_eq!(action.title, "Omit 2 telemetry channels");
    assert!(
        apply_only_fix(&text, &actions).ends_with(
            "\
  } omit {
    c1.T
    c2.T
  }
}
"
        ),
        "unexpected fix result:\n{}",
        apply_only_fix(&text, &actions)
    );
}

#[test]
fn no_quick_fix_when_every_channel_is_covered() {
    let top = "\
deployment topology T {
  instance c1
  instance c2
  telemetry packets P {
    packet P group 0 {
      c1.T
      c2.T
    }
  }
}
";
    let (_, actions) = code_actions_at("ws_omit_covered", top, 3, 4);
    assert!(
        actions.is_empty(),
        "a packet set that covers its dictionary should offer no fix, got {actions:?}"
    );
}
