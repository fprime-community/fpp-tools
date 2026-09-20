use crate::Diagnosed;
use fpp_ast::{MoveWalkable, MutVisitor, Node, TransUnit, Visitor};
use fpp_core::{SourceFile, Spanned};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

pub struct Input {
    pub uri: String,
    pub content: String,
}

/// Must be called outside the `fpp_core::run` scope.
pub fn read_one(path: &Path) -> Result<Input, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(Input {
        uri: path.to_string_lossy().into_owned(),
        content,
    })
}

/// Must be called outside the `fpp_core::run` scope.
pub fn read(paths: &[PathBuf]) -> Result<Vec<Input>, String> {
    let mut seen: Vec<&Path> = Vec::new();
    let mut inputs = Vec::with_capacity(paths.len());
    for path in paths {
        // A `SourceFile` is keyed by its URI; a second one with the same URI
        // discards the first, along with its spans and nodes.
        if seen.contains(&path.as_path()) {
            continue;
        }
        seen.push(path);
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        inputs.push(Input {
            uri: path.to_string_lossy().into_owned(),
            content,
        });
    }
    if inputs.is_empty() {
        return Err("no input files given".to_string());
    }
    Ok(inputs)
}

/// Parse each input into a translation unit and splice in its `include`s. Those
/// are read from disk, unrelated to the `-i` imports the build-time invocation
/// passes.
///
/// Must be called inside a `fpp_core::run` scope. Diagnostics for syntax errors
/// and unreadable includes are emitted through the context.
pub fn parse(inputs: Vec<Input>) -> Vec<TransUnit> {
    let resolver = fpp_parser::ResolveIncludes::new(fpp_fs::FsReader {});
    inputs
        .into_iter()
        .map(|input| {
            let source = SourceFile::new(&input.uri, input.content);
            let mut unit = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut include_context = Default::default();
            let _ = resolver.visit_trans_unit(&mut include_context, &mut unit);
            unit
        })
        .collect()
}

/// Run the expansion passes
pub fn expand(units: &mut [TransUnit]) {
    for unit in units {
        fpp_parser::add_state_enums(unit);
    }
}

/// Fail if any `include` specifier survived resolution, which means definitions in
/// the model are invisible to this tool.
///
/// `fpp_parser::ResolveIncludes` handles `include` inside a module, component,
/// topology, telemetry packet, and telemetry packet set. It does not handle
/// `state machine` or `state` bodies, so those are what this catches.
pub fn check_unresolved_includes(units: &[TransUnit]) -> Result<(), Diagnosed> {
    struct FindInclude;
    impl<'ast> Visitor<'ast> for FindInclude {
        type Break = ();
        type State = bool;

        fn super_visit(&self, found: &mut bool, node: Node<'ast>) -> ControlFlow<()> {
            if let Node::SpecInclude(spec) = node {
                spec.span()
                    .error(
                        "this `include` was not resolved, so definitions inside it are invisible",
                    )
                    .note(format!(
                        "cannot read the members of `{}` here",
                        spec.file.data
                    ))
                    .note(
                        "`include` inside a `state machine` or `state` body is not yet supported; \
                         move those definitions into the file that needs them",
                    )
                    .emit();
                *found = true;
            }
            node.walk(found, self)
        }
    }

    let mut found = false;
    let _ = FindInclude.visit_trans_units(&mut found, units.iter());
    if found { Err(Diagnosed) } else { Ok(()) }
}
