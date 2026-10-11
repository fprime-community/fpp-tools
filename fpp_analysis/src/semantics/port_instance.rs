use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Symbol, SymbolInterface};
use fpp_ast::{
    AstNode, Expr, FormalParam, GeneralPortInstanceKind, InputPortKind, QueueFull,
    SpecGeneralPortInstance, SpecInternalPort, SpecPortInstance, SpecSpecialPortInstance,
    SpecTopPort, SpecialPortInstanceKind,
};
use fpp_core::{Node, Span, Spanned};
use std::sync::Arc;

/// An FPP port instance signature: the parts of a port instance that must
/// agree for one port instance to stand in for another
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PortInstanceSignature {
    direction: Option<Direction>,
    array_size: i128,
    ty: Option<PortInstanceType>,
    name: String,
}

/// An FPP port instance.
///
/// The payload of each variant is its own struct, so that code which requires a
/// particular kind of port instance can say so in its types.
#[derive(Debug, Clone)]
pub enum PortInstance {
    /// A general port instance
    General(GeneralPortInstance),
    /// A special port instance
    Special(SpecialPortInstance),
    /// An internal port instance
    Internal(InternalPortInstance),
    /// A topology port instance
    Topology(TopologyPortInstance),
}

impl PortInstance {
    /// Gets the size of the port array
    pub fn get_array_size(&self) -> i128 {
        match self {
            PortInstance::General(pi) => pi.get_array_size(),
            PortInstance::Topology(pi) => pi.underlying_port.get_array_size(),
            PortInstance::Special(_) | PortInstance::Internal(_) => 1,
        }
    }

    /// Gets the direction of the port instance
    pub fn get_direction(&self) -> Option<Direction> {
        match self {
            PortInstance::General(pi) => Some(pi.get_direction()),
            PortInstance::Topology(pi) => pi.underlying_port.get_direction(),
            PortInstance::Special(pi) => Some(pi.get_direction()),
            PortInstance::Internal(_) => None,
        }
    }

    /// Gets the type of the port instance
    pub fn get_type(&self) -> Option<PortInstanceType> {
        match self {
            PortInstance::General(pi) => Some(pi.get_type()),
            PortInstance::Topology(pi) => pi.underlying_port.get_type(),
            PortInstance::Special(pi) => Some(pi.get_type()),
            PortInstance::Internal(_) => None,
        }
    }

    /// Gets the location of the port instance
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the AST node ID of the port instance
    pub fn get_node_id(&self) -> Node {
        match self {
            PortInstance::General(pi) => pi.get_node_id(),
            PortInstance::Topology(pi) => pi.get_node_id(),
            PortInstance::Special(pi) => pi.get_node_id(),
            PortInstance::Internal(pi) => pi.get_node_id(),
        }
    }

    /// Gets the special kind of the port instance, if any
    pub fn get_special_kind(&self) -> Option<SpecialPortInstanceKind> {
        match self {
            PortInstance::Special(pi) => Some(pi.get_special_kind()),
            PortInstance::General(_) | PortInstance::Topology(_) | PortInstance::Internal(_) => {
                None
            }
        }
    }

    /// Gets the locations of the import specifiers (if this port was imported).
    /// The first item is the import of the parent interface. The final item is
    /// the import into the component. All the in-between locations are for
    /// imports into other interfaces.
    pub fn get_import_locs(&self) -> Vec<Span> {
        get_import_locs(self.get_import_node_ids())
    }

    /// Gets the node IDs of the import specifiers (if this port was imported),
    /// in the order of [`PortInstance::get_import_locs`]
    pub fn get_import_node_ids(&self) -> &[Node] {
        match self {
            PortInstance::General(pi) => pi.get_import_node_ids(),
            PortInstance::Special(pi) => pi.get_import_node_ids(),
            // Internal and topology ports cannot be imported
            PortInstance::Topology(_) | PortInstance::Internal(_) => &[],
        }
    }

    /// Returns a copy of this port instance imported through `import_node`
    pub fn with_import_specifier(&self, import_node: Node) -> PortInstance {
        match self {
            PortInstance::General(pi) => {
                PortInstance::General(pi.with_import_specifier(import_node))
            }
            PortInstance::Special(pi) => {
                PortInstance::Special(pi.with_import_specifier(import_node))
            }
            // Internal and topology ports cannot be imported
            PortInstance::Topology(_) | PortInstance::Internal(_) => self.clone(),
        }
    }

    /// Gets the unqualified name of the port instance
    pub fn get_unqualified_name(&self) -> &str {
        match self {
            PortInstance::General(pi) => pi.get_unqualified_name(),
            PortInstance::Topology(pi) => pi.get_unqualified_name(),
            PortInstance::Special(pi) => pi.get_unqualified_name(),
            PortInstance::Internal(pi) => pi.get_unqualified_name(),
        }
    }

    /// Checks whether a port instance may be connected.
    ///
    /// `loc` is the location where the connection is requested.
    pub fn require_connection_at(&self, loc: Span) -> SemanticResult {
        match self {
            PortInstance::Internal(pi) => Err(SemanticError::InvalidPortKind {
                loc,
                msg: "cannot connect to internal port".to_string(),
                spec_loc: pi.get_loc(),
            }),
            _ => Ok(()),
        }
    }

    /// Builds a topology port aliasing an underlying port instance
    pub fn topology(node: Arc<SpecTopPort>, underlying_port: PortInstance) -> PortInstance {
        PortInstance::Topology(TopologyPortInstance {
            node,
            underlying_port: Box::new(underlying_port),
        })
    }

    /// Whether this port instance is an async input (general async, special
    /// async, or internal). Rust-only, for the Python bindings.
    pub fn is_async_input(&self) -> bool {
        match self {
            PortInstance::General(pi) => pi.is_async_input(),
            PortInstance::Topology(pi) => pi.underlying_port.is_async_input(),
            PortInstance::Special(pi) => pi.is_async_input(),
            PortInstance::Internal(_) => true,
        }
    }

    pub(crate) fn signature(&self) -> PortInstanceSignature {
        match self {
            PortInstance::General(pi) => pi.signature(),
            PortInstance::Topology(pi) => pi.signature(),
            PortInstance::Special(pi) => pi.signature(),
            PortInstance::Internal(pi) => pi.signature(),
        }
    }

    /// Whether two port instances have the same connection signature
    pub fn signature_eq(&self, other: &PortInstance) -> bool {
        self.signature() == other.signature()
    }

    /// Creates a port instance from an internal port specifier
    pub fn from_spec_internal_port(
        a: &Analysis,
        node: &SpecInternalPort,
    ) -> SemanticResult<InternalPortInstance> {
        let loc = node.span();
        let check_ref_params = |params: &[FormalParam]| {
            if Analysis::get_num_ref_params(params) != 0 {
                Err(SemanticError::InvalidInternalPort {
                    loc,
                    msg: "internal port may not have ref parameters".to_string(),
                })
            } else {
                Ok(())
            }
        };
        let priority = a.get_big_int_value_opt(&node.priority);
        Analysis::check_for_duplicate_parameter(&node.params)?;
        check_ref_params(&node.params)?;
        Ok(InternalPortInstance {
            node: Arc::new(node.clone()),
            priority,
            queue_full: Analysis::get_queue_full(&node.queue_full),
        })
    }

    /// Creates a general port instance
    fn create_general_port_instance(
        a: &Analysis,
        specifier: &SpecGeneralPortInstance,
    ) -> SemanticResult<GeneralPortInstance> {
        if !matches!(
            specifier.kind,
            GeneralPortInstanceKind::Input(InputPortKind::Async)
        ) {
            // Check the priority specifier
            if let Some(priority) = &specifier.priority {
                return Err(SemanticError::InvalidPriority {
                    loc: priority.span(),
                });
            }
            // Check the queue full specifier
            if let Some(queue_full) = &specifier.queue_full {
                return Err(SemanticError::InvalidQueueFull {
                    loc: queue_full.span(),
                });
            }
        }
        // Get the size
        let size = get_array_size(a, &specifier.size)?;
        // Get the priority
        let priority = a.get_big_int_value_opt(&specifier.priority);
        // Get the type
        let ty = match &specifier.port {
            Some(qid) => match a.use_def_map.get(&qid.id()) {
                Some(Symbol::Port(symbol)) => PortInstanceType::DefPort(symbol.clone()),
                Some(symbol) => {
                    return Err(SemanticError::InvalidSymbol {
                        symbol_name: symbol.name().data.clone(),
                        loc: qid.span(),
                        msg: "not a port symbol".to_string(),
                        def_loc: symbol.node().span(),
                    });
                }
                None => PortInstanceType::Serial,
            },
            None => PortInstanceType::Serial,
        };
        let kind = match &specifier.kind {
            GeneralPortInstanceKind::Input(InputPortKind::Async) => GeneralKind::AsyncInput {
                priority,
                queue_full: Analysis::get_specified_queue_full(&specifier.queue_full),
            },
            GeneralPortInstanceKind::Input(InputPortKind::Guarded) => GeneralKind::GuardedInput,
            GeneralPortInstanceKind::Output => GeneralKind::Output,
            GeneralPortInstanceKind::Input(InputPortKind::Sync) => GeneralKind::SyncInput,
        };
        Ok(GeneralPortInstance {
            node: Arc::new(specifier.clone()),
            kind,
            size,
            ty,
            import_node_ids: vec![],
        })
    }

    /// Creates a special port instance
    pub fn create_special_port_instance(
        a: &Analysis,
        specifier: &SpecSpecialPortInstance,
    ) -> SemanticResult<SpecialPortInstance> {
        let loc = specifier.span();
        let def = match a.use_def_map.get(&specifier.node_id) {
            Some(Symbol::Port(def)) => def.clone(),
            // The implied use names a symbol of another kind, such as a module
            // Fw.Time
            Some(symbol) => {
                return Err(SemanticError::InvalidSymbol {
                    symbol_name: symbol.name().data.clone(),
                    loc,
                    msg: "not a port symbol".to_string(),
                    def_loc: symbol.node().span(),
                });
            }
            None => {
                return Err(SemanticError::InvalidSpecialPort {
                    loc,
                    msg: "not a port symbol".to_string(),
                });
            }
        };
        let kind_string = specifier.kind.to_string();
        // Check the input kind
        match (&specifier.input_kind, &specifier.kind) {
            (Some(_), SpecialPortInstanceKind::ProductRecv) => {}
            (Some(_), _) => {
                return Err(SemanticError::InvalidSpecialPort {
                    loc,
                    msg: format!("{} port may not specify input kind", kind_string),
                });
            }
            (None, SpecialPortInstanceKind::ProductRecv) => {
                return Err(SemanticError::InvalidSpecialPort {
                    loc,
                    msg: format!("{} port must specify input kind", kind_string),
                });
            }
            _ => {}
        }
        if !matches!(specifier.input_kind, Some(InputPortKind::Async)) {
            // Check the priority specifier
            if let Some(priority) = &specifier.priority {
                return Err(SemanticError::InvalidPriority {
                    loc: priority.span(),
                });
            }
            // Check the queue full specifier
            if let Some(queue_full) = &specifier.queue_full {
                return Err(SemanticError::InvalidQueueFull {
                    loc: queue_full.span(),
                });
            }
        }
        // Get the priority
        let priority = a.get_big_int_value_opt(&specifier.priority);
        let queue_full = match specifier.kind {
            SpecialPortInstanceKind::ProductRecv => {
                Some(Analysis::get_specified_queue_full(&specifier.queue_full))
            }
            _ => None,
        };
        Ok(SpecialPortInstance {
            node: Arc::new(specifier.clone()),
            def,
            priority,
            queue_full,
            import_node_ids: vec![],
        })
    }

    /// Creates a port instance from a port instance specifier
    pub fn from_spec_port_instance(
        a: &Analysis,
        node: &SpecPortInstance,
    ) -> SemanticResult<PortInstance> {
        match node {
            SpecPortInstance::General(specifier) => {
                let instance = PortInstance::create_general_port_instance(a, specifier)?;
                check_general_async_input(&instance)?;
                Ok(PortInstance::General(instance))
            }
            SpecPortInstance::Special(specifier) => Ok(PortInstance::Special(
                PortInstance::create_special_port_instance(a, specifier)?,
            )),
        }
    }
}

/// Gets an array size from an AST node
fn get_array_size(a: &Analysis, size_opt: &Option<Expr>) -> SemanticResult<i128> {
    match size_opt {
        Some(size) => a.get_array_size(size.node_id, size.span()),
        None => Ok(1),
    }
}

/// Checks general async input port specifiers
fn check_general_async_input(instance: &GeneralPortInstance) -> SemanticResult {
    let loc = instance.get_loc();
    if let GeneralKind::AsyncInput { .. } = instance.kind
        && let PortInstanceType::DefPort(def_port) = &instance.ty
        && def_port.return_type.is_some()
    {
        return Err(SemanticError::InvalidPortInstance {
            loc,
            msg: "async input port may not return a value".to_string(),
            def_loc: def_port.span(),
        });
    }
    Ok(())
}

/// Gets the locations of a list of import specifier node IDs
fn get_import_locs(import_node_ids: &[Node]) -> Vec<Span> {
    import_node_ids.iter().map(Spanned::span).collect()
}

/// A port instance type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortInstanceType {
    DefPort(Arc<fpp_ast::DefPort>),
    Serial,
}

impl PortInstanceType {
    /// Show a type option. A defined port shows as the qualified name of its
    /// definition.
    pub fn show(a: &Analysis, ty: &Option<PortInstanceType>) -> String {
        match ty {
            Some(PortInstanceType::DefPort(port)) => {
                a.get_qualified_name(&Symbol::Port(port.clone()))
            }
            Some(PortInstanceType::Serial) => "serial".to_string(),
            None => "none".to_string(),
        }
    }

    /// Check whether types are compatible
    pub fn are_compatible(t1: &Option<PortInstanceType>, t2: &Option<PortInstanceType>) -> bool {
        match (t1, t2) {
            (Some(PortInstanceType::Serial), _) => true,
            (_, Some(PortInstanceType::Serial)) => true,
            (Some(t1), Some(t2)) => t1 == t2,
            _ => false,
        }
    }

    /// If this is a defined port with a return type, gets the location of the
    /// port definition. Rust-only, for the connection checks.
    pub fn port_returns_value(&self) -> Option<Span> {
        match self {
            PortInstanceType::DefPort(def) => def.return_type.as_ref().map(|_| def.span()),
            _ => None,
        }
    }
}

impl std::fmt::Display for PortInstanceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortInstanceType::DefPort(port) => f.write_str(&port.name.data),
            PortInstanceType::Serial => f.write_str("serial"),
        }
    }
}

/// A port direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

impl Direction {
    /// Show a direction option
    pub fn show(dir: &Option<Direction>) -> &'static str {
        match dir {
            Some(Direction::Input) => "input",
            Some(Direction::Output) => "output",
            None => "none",
        }
    }

    /// Check whether directions are compatible
    pub fn are_compatible(from: &Option<Direction>, to: &Option<Direction>) -> bool {
        matches!(
            (from, to),
            (Some(Direction::Output), Some(Direction::Input))
        )
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Direction::show(&Some(*self)))
    }
}

/// A general port kind
#[derive(Debug, Clone)]
pub enum GeneralKind {
    AsyncInput {
        priority: Option<i128>,
        queue_full: QueueFull,
    },
    GuardedInput,
    Output,
    SyncInput,
}

/// A general port instance
#[derive(Debug, Clone)]
pub struct GeneralPortInstance {
    /// The specifier defining the port instance
    pub node: Arc<SpecGeneralPortInstance>,
    /// The kind of the general port
    pub kind: GeneralKind,
    /// The size of the port array
    pub size: i128,
    /// The type of the port
    pub ty: PortInstanceType,
    /// The node IDs of the import specifiers this port came through
    pub import_node_ids: Vec<Node>,
}

impl GeneralPortInstance {
    /// Gets the direction of the port instance. A general port always has one.
    pub fn get_direction(&self) -> Direction {
        match self.kind {
            GeneralKind::Output => Direction::Output,
            _ => Direction::Input,
        }
    }

    /// Gets the size of the port array
    pub fn get_array_size(&self) -> i128 {
        self.size
    }

    /// Gets the node ID of the port instance
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
    }

    /// Gets the location of the port instance
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the type of the port instance. A general port always has one.
    pub fn get_type(&self) -> PortInstanceType {
        self.ty.clone()
    }

    /// Gets the unqualified name of the port instance
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    /// Returns a copy of this port instance imported through `import_node`
    pub fn with_import_specifier(&self, import_node: Node) -> GeneralPortInstance {
        let mut result = self.clone();
        result.import_node_ids.push(import_node);
        result
    }

    /// Gets the node IDs of the import specifiers (if this port was imported)
    pub fn get_import_node_ids(&self) -> &[Node] {
        &self.import_node_ids
    }

    /// Gets the locations of the import specifiers (if this port was imported)
    pub fn get_import_locs(&self) -> Vec<Span> {
        get_import_locs(self.get_import_node_ids())
    }

    /// Whether this port instance is an async input
    pub fn is_async_input(&self) -> bool {
        matches!(self.kind, GeneralKind::AsyncInput { .. })
    }

    fn signature(&self) -> PortInstanceSignature {
        PortInstanceSignature {
            direction: Some(self.get_direction()),
            array_size: self.get_array_size(),
            ty: Some(self.get_type()),
            name: self.get_unqualified_name().to_string(),
        }
    }
}

/// A topology port instance, aliasing an underlying port instance
#[derive(Debug, Clone)]
pub struct TopologyPortInstance {
    /// The specifier defining the topology port
    pub node: Arc<SpecTopPort>,
    /// The port instance this topology port aliases
    pub underlying_port: Box<PortInstance>,
}

impl TopologyPortInstance {
    /// Gets the node ID of the port instance
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
    }

    /// Gets the location of the port instance
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the unqualified name of the port instance
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    fn signature(&self) -> PortInstanceSignature {
        // The topology port takes its own name, but everything else from the
        // port it aliases.
        PortInstanceSignature {
            name: self.get_unqualified_name().to_string(),
            ..self.underlying_port.signature()
        }
    }
}

/// A special port instance
#[derive(Debug, Clone)]
pub struct SpecialPortInstance {
    /// The specifier defining the port instance
    pub node: Arc<SpecSpecialPortInstance>,
    /// The definition of the port that the special port's implied use names
    pub def: Arc<fpp_ast::DefPort>,
    /// The queue priority, for an async special port
    pub priority: Option<i128>,
    /// The queue-full behavior, for a product receive port
    pub queue_full: Option<QueueFull>,
    /// The node IDs of the import specifiers this port came through
    pub import_node_ids: Vec<Node>,
}

impl SpecialPortInstance {
    /// Gets the direction of the port instance. A special port always has one.
    pub fn get_direction(&self) -> Direction {
        match self.node.kind {
            SpecialPortInstanceKind::CommandRecv | SpecialPortInstanceKind::ProductRecv => {
                Direction::Input
            }
            _ => Direction::Output,
        }
    }

    /// Gets the node ID of the port instance
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
    }

    /// Gets the location of the port instance
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the type of the port instance. A special port always has one.
    pub fn get_type(&self) -> PortInstanceType {
        PortInstanceType::DefPort(self.def.clone())
    }

    /// Gets the unqualified name of the port instance
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the special kind of the port instance
    pub fn get_special_kind(&self) -> SpecialPortInstanceKind {
        self.node.kind.clone()
    }

    /// Returns a copy of this port instance imported through `import_node`
    pub fn with_import_specifier(&self, import_node: Node) -> SpecialPortInstance {
        let mut result = self.clone();
        result.import_node_ids.push(import_node);
        result
    }

    /// Gets the node IDs of the import specifiers (if this port was imported)
    pub fn get_import_node_ids(&self) -> &[Node] {
        &self.import_node_ids
    }

    /// Gets the locations of the import specifiers (if this port was imported)
    pub fn get_import_locs(&self) -> Vec<Span> {
        get_import_locs(self.get_import_node_ids())
    }

    /// Whether this port instance is an async input
    pub fn is_async_input(&self) -> bool {
        matches!(self.node.input_kind, Some(InputPortKind::Async))
    }

    pub(crate) fn signature(&self) -> PortInstanceSignature {
        PortInstanceSignature {
            direction: Some(self.get_direction()),
            array_size: 1,
            ty: Some(self.get_type()),
            name: self.get_unqualified_name().to_string(),
        }
    }
}

/// An internal port instance. Internal ports cannot be imported, so they carry
/// no import specifiers.
#[derive(Debug, Clone)]
pub struct InternalPortInstance {
    /// The specifier defining the port instance
    pub node: Arc<SpecInternalPort>,
    /// The queue priority
    pub priority: Option<i128>,
    /// The queue-full behavior
    pub queue_full: QueueFull,
}

impl InternalPortInstance {
    /// Gets the node ID of the port instance
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
    }

    /// Gets the location of the port instance
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the unqualified name of the port instance
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    fn signature(&self) -> PortInstanceSignature {
        PortInstanceSignature {
            direction: None,
            array_size: 1,
            ty: None,
            name: self.get_unqualified_name().to_string(),
        }
    }
}

// A port instance shows as its unqualified name, except for a topology port,
// which also shows the port it aliases.

impl std::fmt::Display for PortInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortInstance::General(pi) => pi.fmt(f),
            PortInstance::Topology(pi) => pi.fmt(f),
            PortInstance::Special(pi) => pi.fmt(f),
            PortInstance::Internal(pi) => pi.fmt(f),
        }
    }
}

impl std::fmt::Display for GeneralPortInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get_unqualified_name())
    }
}

impl std::fmt::Display for TopologyPortInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} -> {}",
            self.get_unqualified_name(),
            self.underlying_port
        )
    }
}

impl std::fmt::Display for SpecialPortInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get_unqualified_name())
    }
}

impl std::fmt::Display for InternalPortInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get_unqualified_name())
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, PortInstance, PortInstanceType};
    use crate::semantics::{Component, PortInterface, Topology};
    use crate::{Analysis, add_state_enums, check_semantics, resolve_templates};
    use fpp_ast::SpecialPortInstanceKind;
    use fpp_core::{SourceFile, Spanned};

    /// A component whose ports cover every kind that can appear in a component's
    /// port interface, plus a topology that aliases one of them.
    const SRC: &str = r#"
module Fw {
  port Cmd
  port CmdReg
  port CmdResponse
}

module M {

  port P

  @ A component with one port of every kind
  active component C1 {

    output port pOut: P

    async input port pIn: P

    internal port pInternal

    command recv port cmdIn

    command reg port cmdRegOut

    command resp port cmdRespOut

  }

  passive component C2 {

    sync input port pIn: P

  }

  instance c1: C1 base id 0x100 queue size 10
  instance c2: C2 base id 0x200

  topology T {

    instance c1
    instance c2

    port a = c1.pOut

    connections C {
      c1.pOut -> c2.pIn
    }

  }

}
"#;

    /// Analyze [`SRC`] and hand the resolved component `C1` and topology `T` to
    /// `f`. Panics if the input produced any diagnostic.
    fn with_analysis(f: impl FnOnce(&Component, &Topology)) {
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let source = SourceFile::new("interface_test.fpp", SRC.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            add_state_enums(&mut ast);
            let _ = resolve_templates(&mut a, std::slice::from_mut(&mut ast));
            let _ = check_semantics(&mut a, vec![&ast]);
            let component = a
                .component_map
                .values()
                .find(|c| c.node.name.data == "C1")
                .expect("component C1 was resolved");
            let topology = a
                .topology_map
                .values()
                .find(|t| t.get_name() == "T")
                .expect("topology T was resolved");
            f(component, topology);
        });
        let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
        assert_eq!(output, "", "expected no diagnostics");
    }

    fn port(port_interface: &PortInterface, name: &str) -> PortInstance {
        port_interface
            .port_map
            .get(name)
            .unwrap_or_else(|| panic!("port {} was resolved", name))
            .clone()
    }

    /// Every port instance shows as its unqualified name, except a topology
    /// port, which also shows the port it aliases.
    #[test]
    fn display_is_the_unqualified_name() {
        with_analysis(|component, topology| {
            let pi = &component.port_interface;
            assert!(matches!(port(pi, "pOut"), PortInstance::General(_)));
            assert_eq!(port(pi, "pOut").to_string(), "pOut");
            assert!(matches!(port(pi, "cmdIn"), PortInstance::Special(_)));
            assert_eq!(port(pi, "cmdIn").to_string(), "cmdIn");
            assert!(matches!(port(pi, "pInternal"), PortInstance::Internal(_)));
            assert_eq!(port(pi, "pInternal").to_string(), "pInternal");

            let alias = port(&topology.port_interface, "a");
            assert!(matches!(alias, PortInstance::Topology(_)));
            assert_eq!(alias.to_string(), "a -> pOut");
        })
    }

    /// A topology port alias takes its name from itself and everything else
    /// from the port it aliases.
    #[test]
    fn topology_port_delegates_to_the_underlying_port() {
        with_analysis(|_, topology| {
            let PortInstance::Topology(alias) = port(&topology.port_interface, "a") else {
                panic!("port a is a topology port");
            };
            assert_eq!(alias.get_unqualified_name(), "a");
            assert_eq!(alias.underlying_port.get_unqualified_name(), "pOut");
            assert_eq!(alias.get_loc(), alias.node.node_id.span());
            let underlying = &*alias.underlying_port;
            let aliased = PortInstance::Topology(alias.clone());
            assert_eq!(aliased.get_direction(), Some(Direction::Output));
            assert_eq!(aliased.get_array_size(), underlying.get_array_size());
            assert_eq!(aliased.get_type(), underlying.get_type());
            // Only the name differs, so the alias does not stand in for the
            // port it aliases.
            assert!(!aliased.signature_eq(underlying));
        })
    }

    /// The special port map holds special port instances, so the special kind
    /// of an entry is available without a fallible match.
    #[test]
    fn special_port_map_holds_special_port_instances() {
        with_analysis(|component, _| {
            let map = component.special_port_map();
            assert_eq!(map.len(), 3);
            for (kind, pi) in map {
                assert_eq!(&pi.get_special_kind(), kind);
                // The same instance is in the port map under its own name.
                assert_eq!(
                    port(&component.port_interface, pi.get_unqualified_name()).get_node_id(),
                    pi.get_node_id()
                );
            }
            let cmd_in = map
                .get(&SpecialPortInstanceKind::CommandRecv)
                .expect("the command recv port was resolved");
            assert_eq!(cmd_in.get_unqualified_name(), "cmdIn");
            assert_eq!(cmd_in.get_direction(), Direction::Input);
        })
    }

    /// An internal port has neither a direction nor a type, and may not be
    /// connected.
    #[test]
    fn internal_port_may_not_be_connected() {
        with_analysis(|component, _| {
            let pi = port(&component.port_interface, "pInternal");
            assert_eq!(pi.get_direction(), None);
            assert_eq!(pi.get_type(), None);
            assert_eq!(pi.get_array_size(), 1);
            assert!(pi.get_import_node_ids().is_empty());
            // An internal port cannot be imported, so an import specifier
            // leaves it unchanged.
            let node = pi.get_node_id();
            assert_eq!(pi.with_import_specifier(node).get_import_locs(), vec![]);
            assert!(pi.require_connection_at(pi.get_loc()).is_err());
        })
    }

    /// A port interface keeps its ports in the order they were added, so the
    /// checks that walk it report the first offending port in that order.
    #[test]
    fn ports_are_walked_in_insertion_order() {
        with_analysis(|component, _| {
            let pi = &component.port_interface;
            let names: Vec<&str> = pi.port_map.keys().map(String::as_str).collect();
            // The component declares no imports, so it holds its ports in the
            // order it declares them.
            assert_eq!(
                names,
                vec![
                    "pOut",
                    "pIn",
                    "pInternal",
                    "cmdIn",
                    "cmdRegOut",
                    "cmdRespOut"
                ]
            );
            let kinds: Vec<SpecialPortInstanceKind> = pi.special_port_map.keys().cloned().collect();
            assert_eq!(
                kinds,
                vec![
                    SpecialPortInstanceKind::CommandRecv,
                    SpecialPortInstanceKind::CommandReg,
                    SpecialPortInstanceKind::CommandResp
                ]
            );
        })
    }

    /// A port type shows as the qualified name of its definition, so two port
    /// definitions with the same unqualified name are told apart in a type
    /// mismatch; the type's Display is the unqualified name.
    #[test]
    fn port_types_show_their_qualified_names() {
        const PORTS: &str = r#"
module M { port P }
module N { port P }

passive component C1 { output port pOut: M.P }
passive component C2 { sync input port pIn: N.P }

instance c1: C1 base id 0x100
instance c2: C2 base id 0x200

topology T {
  instance c1
  instance c2
  connections X { c1.pOut -> c2.pIn }
}
"#;
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let source = SourceFile::new("port_types.fpp", PORTS.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            add_state_enums(&mut ast);
            let _ = resolve_templates(&mut a, std::slice::from_mut(&mut ast));
            let _ = check_semantics(&mut a, vec![&ast]);
            let component = a
                .component_map
                .values()
                .find(|c| c.node.name.data == "C1")
                .expect("component C1 was resolved");
            let ty = port(&component.port_interface, "pOut").get_type();
            assert_eq!(PortInstanceType::show(&a, &ty), "M.P");
            assert_eq!(ty.expect("pOut has a type").to_string(), "P");
            assert_eq!(PortInstanceType::show(&a, &None), "none");
        });
        let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
        assert!(
            output.contains("cannot connect port types M.P and N.P"),
            "unexpected diagnostics: {output}"
        );
    }
}
