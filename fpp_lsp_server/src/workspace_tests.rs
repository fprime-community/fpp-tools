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

/// Index a single-file project and open the file in the VFS, so completion can
/// read the document text.
fn state_with_open_document(dir: &Path, text: &str) -> (GlobalState, String) {
    std::fs::write(dir.join("Top.fpp"), text).unwrap();
    std::fs::write(dir.join(".fpp-lsp"), "scanWorkspace: true\n").unwrap();

    let mut state = index_workspace(dir);
    let uri = crate::uri::from_file_path(dir.join("Top.fpp")).unwrap();
    state.vfs.did_open(lsp_types::DidOpenTextDocumentParams {
        text_document: lsp_types::TextDocumentItem {
            uri: Uri::from_str(&uri).unwrap(),
            language_id: "fpp".into(),
            version: 1,
            text: text.to_string(),
        },
    });
    (state, uri)
}

/// The completion item labels offered at a zero-based `line`/`character`.
fn completion_labels_at(state: &GlobalState, uri: &str, line: u32, character: u32) -> Vec<String> {
    completion_items(
        crate::handlers::handle_completion(state, completion_params_at(uri, line, character))
            .expect("completion should not error"),
    )
    .into_iter()
    .map(|i| i.label)
    .collect()
}

/// An expansion specifier names a template, so completion there must offer the
/// template name group — both for a bare name and after a qualifier's dot.
/// Nothing else lives in that name group, so no other definition may leak in.
#[test]
fn completion_after_expand_offers_templates() {
    let dir = fixture_dir("ws_expand_completion");
    // Line 4 is `expand `, line 5 is `expand M.`.
    let text = "\
module M {
  module template Tmpl() { }
  constant NotATemplate = 1
}
expand
expand M.
";
    let (state, uri) = state_with_open_document(&dir, text);

    let bare = completion_labels_at(&state, &uri, 4, 7);
    assert!(
        bare.contains(&"M".to_string()),
        "`expand ` should offer the enclosing module as a qualifier, got {bare:?}"
    );
    assert!(
        !bare.contains(&"NotATemplate".to_string()),
        "`expand ` must not offer non-templates, got {bare:?}"
    );

    let qualified = completion_labels_at(&state, &uri, 5, 9);
    assert_eq!(
        qualified,
        vec!["Tmpl".to_string()],
        "`expand M.` should offer only module `M`'s templates"
    );
}

/// The implements clause of a topology is a list of port interface names, so
/// completion after `implements` or after a comma in the list offers port
/// interfaces. The list may be empty, so nothing there is a syntax error.
#[test]
fn completion_in_implements_clause_offers_interfaces() {
    let dir = fixture_dir("ws_implements_completion");
    // Lines 2 and 4 leave a name of the list empty.
    let text = "\
interface Iface { }
passive component Comp { }
topology T implements  {
}
topology U implements Iface,  {
}
";
    let (state, uri) = state_with_open_document(&dir, text);

    for (line, character) in [(2, 22), (4, 29)] {
        let labels = completion_labels_at(&state, &uri, line, character);
        assert!(
            labels.contains(&"Iface".to_string()),
            "line {line}: the implements clause should offer port interfaces, got {labels:?}"
        );
        assert!(
            !labels.contains(&"Comp".to_string()),
            "line {line}: the implements clause must not offer components, got {labels:?}"
        );
    }
}

/// An `instance` template argument is a bare qualified identifier, so completion
/// there must offer interface instances. The other two argument kinds are an
/// expression and a type name, which the generic expression/type paths handle.
#[test]
fn completion_in_instance_template_argument_offers_instances() {
    let dir = fixture_dir("ws_template_arg_completion");
    // Line 7 leaves the `instance` argument's name empty.
    let text = "\
interface Iface { }
passive component Comp { }
instance comp: Comp base id 1
module template Tmpl(instance myInst: Iface) {
  topology Top {
    instance myInst
  }
}
expand Tmpl(instance )
";
    let (state, uri) = state_with_open_document(&dir, text);

    let labels = completion_labels_at(&state, &uri, 7, 21);
    assert!(
        labels.contains(&"comp".to_string()),
        "an `instance` argument should offer component instances, got {labels:?}"
    );
    assert!(
        !labels.contains(&"Tmpl".to_string()),
        "an `instance` argument must not offer templates, got {labels:?}"
    );
}

/// A template parameter is in scope only inside the body of the template that
/// declares it, and only in the name group its kind belongs to. It has no symbol
/// in any enclosing scope, so completion must offer it from the declaration.
#[test]
fn completion_inside_template_body_offers_parameters() {
    let dir = fixture_dir("ws_template_param_completion");
    // Line 6 completes a type name, line 7 a constant expression.
    let text = "\
port P
interface Iface {
  output port pOut: P
}
module template Tmpl(constant Size: U32, type Elem, instance myInst: Iface) {
  array A = [3] U32
  array B = [3]
  constant C =
}
";
    let (state, uri) = state_with_open_document(&dir, text);

    let type_position = completion_labels_at(&state, &uri, 6, 16);
    assert!(
        type_position.contains(&"Elem".to_string()),
        "a type position in a template body should offer the type parameter, got {type_position:?}"
    );
    assert!(
        !type_position.contains(&"Size".to_string()),
        "a type position must not offer the constant parameter, got {type_position:?}"
    );

    let value_position = completion_labels_at(&state, &uri, 7, 15);
    assert!(
        value_position.contains(&"Size".to_string()),
        "a constant position in a template body should offer the constant parameter, got {value_position:?}"
    );
    assert!(
        !value_position.contains(&"Elem".to_string()),
        "a constant position must not offer the type parameter, got {value_position:?}"
    );
}

/// A parameter is not in scope in its own declaration, so the parameter list
/// offers no parameters.
#[test]
fn completion_in_template_parameter_list_offers_no_parameters() {
    let dir = fixture_dir("ws_template_param_list_completion");
    // Line 1 leaves the second parameter's type name empty.
    let text = "\
module template Tmpl(type Elem, constant Size: ) {
  array A = [3] U32
}
";
    let (state, uri) = state_with_open_document(&dir, text);

    let labels = completion_labels_at(&state, &uri, 0, 47);
    assert!(
        !labels.contains(&"Elem".to_string()),
        "a parameter's own declaration must not offer the parameters, got {labels:?}"
    );
    assert!(
        labels.contains(&"U32".to_string()),
        "fixture precondition: a type position offers the primitive types, got {labels:?}"
    );
}

/// Inside a template body an instance parameter is a connection endpoint, but it
/// has no symbol in any enclosing scope. Only the port interface declared in the
/// parameter is visible there, so its ports are what completion offers.
#[test]
fn completion_after_instance_parameter_dot_offers_interface_ports() {
    let dir = fixture_dir("ws_template_instance_param_completion");
    // Line 9 is the connection whose `from` endpoint is the parameter.
    let text = "\
port P
interface Iface {
  output port ifaceOut: P
}
passive component Comp {
  sync input port compIn: P
}
instance comp: Comp base id 1
module template Tmpl(instance myInst: Iface) {
  topology Top {
    instance myInst
    instance comp
    connections C {
      myInst. -> comp.compIn
    }
  }
}
";
    let (state, uri) = state_with_open_document(&dir, text);

    let labels = completion_labels_at(&state, &uri, 13, 13);
    assert_eq!(
        labels,
        vec!["ifaceOut".to_string()],
        "the parameter's declared interface ports should be offered"
    );
}

/// A template body is analyzed once per expansion, so the same line of the body
/// carries one diagnostic per expansion. Each must point at the expansion
/// specifier that produced it, or the editor shows indistinguishable duplicates.
#[test]
fn diagnostics_in_a_template_body_point_at_the_expansion() {
    let dir = fixture_dir("ws_template_diagnostics");
    let text = "\
module template Tmpl() {
  constant C = Undefined
}
module A {
  expand Tmpl()
}
module B {
  expand Tmpl()
}
";
    let (state, uri) = state_with_open_document(&dir, text);

    let diagnostics = state.diagnostics.get(&uri);
    let in_body: Vec<&lsp_types::Diagnostic> = diagnostics
        .iter()
        .filter(|d| d.range.start.line == 1)
        .collect();
    assert_eq!(
        in_body.len(),
        2,
        "expected one diagnostic per expansion, got {diagnostics:?}"
    );

    // Each diagnostic names its own `expand` specifier, so the two are distinct.
    let mut expansion_lines: Vec<u32> = in_body
        .iter()
        .map(|d| {
            let related = d
                .related_information
                .as_ref()
                .expect("related information present");
            let expansion = related
                .iter()
                .find(|r| r.message == "expanded at")
                .unwrap_or_else(|| panic!("no expansion location in {related:?}"));
            expansion.location.range.start.line
        })
        .collect();
    expansion_lines.sort_unstable();
    assert_eq!(expansion_lines, vec![4, 7]);
}

/// The server names its files by `file://` URI, while a location specifier names
/// the file of its definition by a path relative to the specifier. A specifier
/// that names the right file is accepted however its path is spelled, and one
/// that names another file is reported with the path that it names.
#[test]
fn location_specifiers_compare_paths_of_file_uris() {
    let dir = fixture_dir("ws_spec_loc");
    let text = "\
locate constant a at \"Top.fpp\"
locate constant b at \"./sub/../Top.fpp\"
locate constant c at \"../ws_spec_loc/Top.fpp\"
locate constant c at \"Top.fpp\"
locate constant d at \"Other.fpp\"
constant a = 0
constant b = 1
constant c = 2
constant d = 3
";
    let (state, uri) = state_with_open_document(&dir, text);

    let diagnostics = state.diagnostics.get(&uri);
    let messages: Vec<(u32, &str)> = diagnostics
        .iter()
        .map(|d| (d.range.start.line, d.message.as_str()))
        .collect();
    let other = dir.join("Other.fpp");
    assert_eq!(
        messages,
        vec![(
            4,
            format!("incorrect location path {}", other.display()).as_str()
        )]
    );
}

/// A module template body is a module member list, so it may hold `include`
/// specifiers. The document-link visitor must descend into it, or the include in
/// a template body is not a clickable link.
#[test]
fn document_links_reach_includes_in_a_template_body() {
    let dir = fixture_dir("ws_template_document_links");
    std::fs::write(dir.join("Defs.fppi"), "constant A = 1\n").unwrap();
    let text = "\
module template Tmpl() {
  include \"Defs.fppi\"
}
module M {
  include \"Defs.fppi\"
}
";
    let (state, uri) = state_with_open_document(&dir, text);

    let links = crate::handlers::handle_document_link_request(
        &state,
        lsp_types::DocumentLinkParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: Uri::from_str(&uri).unwrap(),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    )
    .expect("document links should not error")
    .unwrap_or_default();

    let lines: Vec<u32> = links.iter().map(|l| l.range.start.line).collect();
    assert_eq!(
        lines,
        vec![1, 4],
        "expected a link for the include in the template body and in the module"
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
