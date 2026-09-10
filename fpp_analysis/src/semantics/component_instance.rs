use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Component, PortInterface, Symbol};
use fpp_ast::{AstNode, ComponentKind, DefComponentInstance, Expr, LitString, SpecInit};
use fpp_core::{Span, Spanned};
use rustc_hash::FxHashMap as HashMap;
use std::path::{Component as PathComponent, Path, PathBuf};
use std::sync::Arc;

/// An FPP init specifier.
#[derive(Debug, Clone)]
pub struct InitSpecifier {
    pub node: Arc<SpecInit>,
    pub phase: i128,
}

impl InitSpecifier {
    /// Gets the location of the init specifier
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates an init specifier from an AST node
    pub fn from_node(a: &Analysis, node: &SpecInit) -> SemanticResult<InitSpecifier> {
        let phase = a.get_int_value_checked(node.phase.node_id, node.phase.span())?;
        Ok(InitSpecifier {
            node: Arc::new(node.clone()),
            phase,
        })
    }
}

/// An FPP component instance.
#[derive(Debug, Clone)]
pub struct ComponentInstance {
    pub node: Arc<DefComponentInstance>,
    pub qualified_name: String,
    pub component_symbol: Symbol,
    pub base_id: i128,
    pub max_id: i128,
    pub file: Option<String>,
    pub queue_size: Option<i128>,
    pub stack_size: Option<i128>,
    pub priority: Option<i128>,
    pub cpu: Option<i128>,
    pub init_specifier_map: HashMap<i128, InitSpecifier>,
}

impl std::fmt::Display for ComponentInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.qualified_name)
    }
}

impl PartialEq for ComponentInstance {
    fn eq(&self, other: &Self) -> bool {
        self.qualified_name == other.qualified_name
    }
}

impl Eq for ComponentInstance {}

impl PartialOrd for ComponentInstance {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ComponentInstance {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.qualified_name.cmp(&other.qualified_name)
    }
}

impl std::hash::Hash for ComponentInstance {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.qualified_name.hash(state);
    }
}

impl ComponentInstance {
    /// Gets the qualified name of the component instance
    pub fn get_qualified_name(&self) -> &str {
        &self.qualified_name
    }

    /// Gets the unqualified name of the component instance
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the component instance
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Gets the component this is an instance of
    pub fn get_component<'a>(&self, a: &'a Analysis) -> Option<&'a Component> {
        a.component_map.get(&self.component_symbol)
    }

    /// Gets the port interface of the component this is an instance of
    pub fn get_interface<'a>(&self, a: &'a Analysis) -> Option<&'a PortInterface> {
        self.get_component(a).map(|c| &c.port_interface)
    }

    /// Adds an init specifier
    pub fn add_init_specifier(&self, spec: InitSpecifier) -> SemanticResult<ComponentInstance> {
        if let Some(prev) = self.init_specifier_map.get(&spec.phase) {
            return Err(SemanticError::DuplicateInitSpecifier {
                phase: spec.phase,
                loc: spec.get_loc(),
                prev_loc: prev.get_loc(),
            });
        }
        let mut ci = self.clone();
        ci.init_specifier_map.insert(spec.phase, spec);
        Ok(ci)
    }

    /// Create a component instance. Returns `None` if the referenced component
    /// is unresolved (CheckUses already reported the error).
    pub fn from_def(
        a: &Analysis,
        node: &DefComponentInstance,
    ) -> SemanticResult<Option<ComponentInstance>> {
        let loc = node.span();
        let name = node.name.data.clone();
        let component = match get_component(a, node) {
            Some(c) => c,
            None => return Ok(None),
        };
        let component_kind = component.node.kind.clone();

        let base_id = match &node.base_id {
            Some(e) => a.get_nonnegative_big_int_value(e.node_id, e.span())?,
            None => 0,
        };

        let file = node.file.as_ref().map(get_file);
        let queue_size = get_queue_size(a, &name, loc, &component_kind, &node.queue_size)?;
        let stack_size = get_active_attribute(
            a,
            &name,
            &component_kind,
            "stack size",
            &node.stack_size,
            true,
        )?;
        let priority =
            get_active_attribute(a, &name, &component_kind, "priority", &node.priority, false)?;
        let cpu =
            get_active_attribute(a, &name, &component_kind, "CPU affinity", &node.cpu, false)?;

        let Some(symbol) = a.get_symbol(node) else {
            return Ok(None);
        };
        let qualified_name = a.get_qualified_name(&symbol);
        let max_id = base_id + component.get_max_id();

        Ok(Some(ComponentInstance {
            node: Arc::new(node.clone()),
            qualified_name,
            component_symbol: component.symbol.clone(),
            base_id,
            max_id,
            file,
            queue_size,
            stack_size,
            priority,
            cpu,
            init_specifier_map: HashMap::default(),
        }))
    }
}

/// The URI scheme that `fpp_lsp_server` uses to identify on-disk source files.
const FILE_URI_SCHEME: &str = "file://";

/// Gets the implementation file of a component instance: the path written in the
/// `at` specifier, resolved against the directory of the source file that
/// specifies it, then lexically normalized (`.` components dropped, `..`
/// cancelled against the preceding name).
///
/// The result is stated in the same frame of reference as that source file's
/// URI: absolute when the URI is absolute, relative to the process working
/// directory when the URI is relative, and a `file://` URI when the source is
/// one. A source URI with no directory part (a bare file name, or the `<stdin>`
/// pseudo-URI) contributes no directory, so the specifier path is used as
/// written. An absolute specifier path replaces the directory outright.
///
/// The path is deliberately not absolutized. The drivers pass source paths
/// through unchanged, and the LSP server caches an `Analysis` and reuses it
/// across requests, so interning the process working directory into stored
/// analysis data would silently misdescribe the model whenever that directory
/// is not the one the paths were given against.
fn get_file(node: &LitString) -> String {
    let uri = node.span().file().uri();
    // The LSP server names source files by `file://` URI. Resolve inside the
    // path part and put the scheme back, so the result stays a well-formed URI
    // rather than a path with a `file:` name component in it.
    let (scheme, path) = match uri.strip_prefix(FILE_URI_SCHEME) {
        Some(path) => (FILE_URI_SCHEME, path),
        // Any other URI — a plain path, or `<stdin>` — is used as written.
        None => ("", uri.as_str()),
    };
    let dir = Path::new(path).parent().unwrap_or(Path::new(""));
    format!(
        "{scheme}{}",
        normalize_path(&dir.join(&node.data)).to_string_lossy()
    )
}

/// Lexically normalizes a path, removing `.` components and resolving `..`
/// against the preceding component, without touching the file system.
fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            PathComponent::CurDir => {}
            PathComponent::ParentDir => {
                // Only pop a component that `..` can cancel out.
                if matches!(
                    result.components().next_back(),
                    Some(PathComponent::Normal(_))
                ) {
                    result.pop();
                } else {
                    result.push(component);
                }
            }
            other => result.push(other),
        }
    }
    result
}

fn get_component(a: &Analysis, node: &DefComponentInstance) -> Option<Component> {
    match a.use_def_map.get(&node.component.id()) {
        Some(symbol @ Symbol::Component(_)) => a.component_map.get(symbol).cloned(),
        _ => None,
    }
}

/// Construct an invalid instance error
fn invalid(name: &str, loc: Span, msg: String) -> SemanticError {
    SemanticError::InvalidDefComponentInstance {
        name: name.to_string(),
        loc,
        msg,
    }
}

/// Gets the queue size
fn get_queue_size(
    a: &Analysis,
    name: &str,
    loc: Span,
    kind: &ComponentKind,
    node_opt: &Option<Expr>,
) -> SemanticResult<Option<i128>> {
    match (kind, node_opt) {
        (ComponentKind::Passive, Some(e)) => Err(invalid(
            name,
            e.span(),
            "passive component may not have queue size".to_string(),
        )),
        (_, Some(e)) => Ok(Some(a.get_nonnegative_big_int_value(e.node_id, e.span())?)),
        (ComponentKind::Passive, None) => Ok(None),
        (kind, None) => Err(invalid(
            name,
            loc,
            format!("{} component must have queue size", kind),
        )),
    }
}

/// Get an attribute for an active component
fn get_active_attribute(
    a: &Analysis,
    name: &str,
    kind: &ComponentKind,
    attr: &str,
    node_opt: &Option<Expr>,
    nonnegative: bool,
) -> SemanticResult<Option<i128>> {
    match (kind, node_opt) {
        (ComponentKind::Active, Some(e)) => {
            if nonnegative {
                Ok(Some(a.get_nonnegative_big_int_value(e.node_id, e.span())?))
            } else {
                Ok(Some(a.get_int_value(e.node_id).unwrap_or(0)))
            }
        }
        (_, Some(e)) => Err(invalid(
            name,
            e.span(),
            format!("{} component may not have {}", kind, attr),
        )),
        (_, None) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{get_active_attribute, get_file};
    use crate::Analysis;
    use fpp_ast::{ComponentKind, Expr, ExprKind, LitString};
    use fpp_core::{CompilerContext, Node, SourceFile, Span};

    /// Runs `f` inside a fresh compiler context, discarding diagnostics.
    fn with_ctx<T>(f: impl FnOnce() -> T) -> T {
        let mut buf = vec![];
        let mut ctx = CompilerContext::new(fpp_errors::WriteEmitter::new(&mut buf));
        fpp_core::run(&mut ctx, f)
    }

    /// The implementation file that an `at "<path>"` specifier in a source file
    /// named `uri` resolves to.
    fn file_for(uri: &str, path: &str) -> String {
        with_ctx(|| {
            let src = SourceFile::new(uri, "instance".to_string());
            // The span machinery requires a non-empty span.
            let span = Span::new(src, 0, 1, None);
            get_file(&LitString {
                data: path.to_string(),
                inner_span: span,
                node_id: Node::new(span),
            })
        })
    }

    #[test]
    fn impl_file_resolves_against_the_specifying_source_directory() {
        assert_eq!(
            file_for("topology/T.fpp", "impl/C.hpp"),
            "topology/impl/C.hpp"
        );
        assert_eq!(file_for("topology/T.fpp", "./C.hpp"), "topology/C.hpp");
        assert_eq!(file_for("topology/T.fpp", "../impl/C.hpp"), "impl/C.hpp");
        assert_eq!(
            file_for("/proj/topology/T.fpp", "../impl/C.hpp"),
            "/proj/impl/C.hpp"
        );
    }

    #[test]
    fn impl_file_keeps_a_file_uri_a_file_uri() {
        assert_eq!(
            file_for("file:///proj/topology/T.fpp", "../impl/C.hpp"),
            "file:///proj/impl/C.hpp"
        );
        assert_eq!(
            file_for("file:///proj/T.fpp", "impl/C.hpp"),
            "file:///proj/impl/C.hpp"
        );
    }

    #[test]
    fn impl_file_for_a_source_with_no_directory_is_the_specifier_path() {
        // `<stdin>` and a bare file name both contribute no directory.
        assert_eq!(file_for("<stdin>", "impl/C.hpp"), "impl/C.hpp");
        assert_eq!(file_for("T.fpp", "./impl/../C.hpp"), "C.hpp");
        // A `..` that has nothing to cancel against is kept.
        assert_eq!(file_for("<stdin>", "../C.hpp"), "../C.hpp");
    }

    #[test]
    fn impl_file_keeps_an_absolute_specifier_path() {
        assert_eq!(
            file_for("topology/T.fpp", "/abs/impl/C.hpp"),
            "/abs/impl/C.hpp"
        );
    }

    /// An expression node with no entry in `Analysis::value_map`, standing for an
    /// attribute that the constant-evaluation passes failed to evaluate.
    fn unevaluated_expr() -> Expr {
        Expr {
            kind: ExprKind::LiteralInt("3".to_string()),
            node_id: Node::new(Span::new(
                SourceFile::new("attr.fpp", "3".to_string()),
                0,
                1,
                None,
            )),
        }
    }

    #[test]
    fn a_specified_active_attribute_is_always_some() {
        with_ctx(|| {
            let a = Analysis::new();
            let e = Some(unevaluated_expr());
            let attribute = |attr: &str, node: &Option<Expr>, nonnegative: bool| {
                get_active_attribute(&a, "c", &ComponentKind::Active, attr, node, nonnegative)
                    .unwrap_or_else(|_| panic!("{attr} is valid for an active component"))
            };
            assert_eq!(attribute("priority", &e, false), Some(0));
            assert_eq!(attribute("CPU affinity", &e, false), Some(0));
            // The nonnegative path (stack size) has the same shape.
            assert_eq!(attribute("stack size", &e, true), Some(0));
            // An unspecified attribute is `None`.
            assert_eq!(attribute("priority", &None, false), None);
        });
    }
}
