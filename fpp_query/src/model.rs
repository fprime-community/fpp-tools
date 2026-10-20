use fpp_ast::{DefModuleTemplate, ModuleMember, MutVisitor, QualIdent, Replicator, TransUnit};
use fpp_core::{SourceFile, Spanned};
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

/// Synthesize the implicit `enum State` of each state machine that has a body, so
/// that the enum `fpp-to-cpp` generates for it is part of the model.
///
/// An external (bodyless) state machine declares no states, so it gets none.
///
/// Reaches a state machine written in a `module template` body or in the members an
/// expansion specifier is written with, and not the members [`expand_templates`]
/// materializes, so it belongs before [`expand_templates`]: run in that order every
/// copy of a template carries the enums the body's state machines were given.
pub fn add_state_enums(units: &mut [TransUnit]) {
    for unit in units {
        fpp_parser::add_state_enums(unit);
    }
}

/// Replace each template expansion specifier with the members of the template it
/// names, so that the definitions an expansion contributes are part of the model.
/// A specifier written with its members, in the form `expand T { ... }`, is already
/// expanded and keeps them.
///
/// The template name is resolved syntactically, by the scoping rules for a
/// qualified identifier: the innermost enclosing module outward. `fpp-check`
/// resolves it through the symbol table instead, but this tool sees one module of
/// a larger build, so a name that does not resolve here simply contributes
/// nothing — the same answer this tool gives for any definition it cannot see.
pub fn expand_templates(units: &mut [TransUnit]) {
    let templates = collect_templates(units);
    for unit in units {
        let mut scope: Vec<String> = Vec::new();
        for member in &mut unit.0 {
            expand_member(&templates, &mut scope, member);
        }
    }
}

/// The module templates of the model, by qualified name
fn collect_templates(units: &[TransUnit]) -> Vec<(Vec<String>, DefModuleTemplate)> {
    fn walk(
        out: &mut Vec<(Vec<String>, DefModuleTemplate)>,
        scope: &mut Vec<String>,
        members: &[ModuleMember],
    ) {
        for member in members {
            match member {
                ModuleMember::DefModule(def) => {
                    scope.push(def.name.data.clone());
                    walk(out, scope, &def.members);
                    scope.pop();
                }
                ModuleMember::DefModuleTemplate(def) => {
                    let mut name = scope.clone();
                    name.push(def.name.data.clone());
                    out.push((name, def.clone()));
                }
                _ => {}
            }
        }
    }

    let mut out = Vec::new();
    for unit in units {
        walk(&mut out, &mut Vec::new(), &unit.0);
    }
    out
}

/// Look up a template by the name a specifier writes, trying the innermost
/// enclosing scope first. An absolute name is looked up in the global scope only.
fn look_up<'t>(
    templates: &'t [(Vec<String>, DefModuleTemplate)],
    scope: &[String],
    name: &QualIdent,
) -> Option<&'t DefModuleTemplate> {
    let ident_list = name.to_ident_list();
    let innermost = if name.is_absolute() { 0 } else { scope.len() };
    for prefix_len in (0..=innermost).rev() {
        let candidate = scope[..prefix_len].iter().chain(&ident_list);
        if let Some((_, def)) = templates
            .iter()
            .find(|(qualified, _)| qualified.iter().eq(candidate.clone()))
        {
            return Some(def);
        }
    }
    None
}

fn expand_member(
    templates: &[(Vec<String>, DefModuleTemplate)],
    scope: &mut Vec<String>,
    member: &mut ModuleMember,
) {
    match member {
        ModuleMember::DefModule(def) => {
            scope.push(def.name.data.clone());
            for member in &mut def.members {
                expand_member(templates, scope, member);
            }
            scope.pop();
        }
        ModuleMember::SpecTemplateExpand(spec) => {
            // An expansion written in the source is already expanded
            if spec.members.is_some() {
                return;
            }
            let Some(def) = look_up(templates, scope, &spec.template) else {
                return;
            };
            let replicator = Replicator::new(spec.span());
            let mut members = def.members.clone();
            replicator.apply(&mut members);
            spec.members = Some(members);
        }
        _ => {}
    }
}
