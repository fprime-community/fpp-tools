use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Symbol, SymbolInterface, cmp_span};
use fpp_ast::{
    AstNode, GeneralPortInstanceKind, InputPortKind, QueueFull, SpecGeneralPortInstance,
    SpecInterfaceImport, SpecInternalPort, SpecSpecialPortInstance, SpecTopPort,
    SpecialPortInstanceKind,
};
use fpp_core::{Node, Span, Spanned};
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

/// A port direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

impl Direction {
    /// Show a direction option.
    pub fn show(dir: &Option<Direction>) -> &'static str {
        match dir {
            Some(Direction::Input) => "input",
            Some(Direction::Output) => "output",
            None => "none",
        }
    }

    /// Directions are compatible iff the connection goes output -> input.
    pub fn are_compatible(from: &Option<Direction>, to: &Option<Direction>) -> bool {
        matches!(
            (from, to),
            (Some(Direction::Output), Some(Direction::Input))
        )
    }
}

/// A port instance type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortInstanceType {
    DefPort(Symbol),
    Serial,
}

impl PortInstanceType {
    /// Show a type option.
    pub fn show(ty: &Option<PortInstanceType>) -> String {
        match ty {
            Some(PortInstanceType::DefPort(symbol)) => symbol.name().data.clone(),
            Some(PortInstanceType::Serial) => "serial".to_string(),
            None => "none".to_string(),
        }
    }

    /// Two types are compatible if either is serial, or they are equal.
    pub fn are_compatible(t1: &Option<PortInstanceType>, t2: &Option<PortInstanceType>) -> bool {
        match (t1, t2) {
            (Some(PortInstanceType::Serial), _) => true,
            (_, Some(PortInstanceType::Serial)) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// If this is a defined port with a return type, get the port def symbol.
    pub fn port_returns_value(&self) -> Option<Span> {
        match self {
            PortInstanceType::DefPort(Symbol::Port(def)) => {
                def.return_type.as_ref().map(|_| def.span())
            }
            _ => None,
        }
    }
}

/// A general port instance kind.
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

/// A general port instance.
#[derive(Debug, Clone)]
pub struct GeneralPortInstance {
    /// The specifier defining the port instance.
    pub node: Arc<SpecGeneralPortInstance>,
    /// The kind of the general port.
    pub kind: GeneralKind,
    /// The size of the port array.
    pub size: i128,
    /// The type of the port.
    pub ty: PortInstanceType,
    /// The node IDs of the import specifiers this port came through.
    pub import_node_ids: Vec<Node>,
}

/// A special port instance.
#[derive(Debug, Clone)]
pub struct SpecialPortInstance {
    /// The specifier defining the port instance.
    pub node: Arc<SpecSpecialPortInstance>,
    /// The port definition symbol backing the special port.
    pub symbol: Symbol,
    /// The queue priority, for an async special port.
    pub priority: Option<i128>,
    /// The queue-full behavior, for a product receive port.
    pub queue_full: Option<QueueFull>,
    /// The node IDs of the import specifiers this port came through.
    pub import_node_ids: Vec<Node>,
}

/// An internal port instance. Internal ports cannot be imported, so they carry
/// no import specifiers.
#[derive(Debug, Clone)]
pub struct InternalPortInstance {
    /// The specifier defining the port instance.
    pub node: Arc<SpecInternalPort>,
    /// The queue priority.
    pub priority: Option<i128>,
    /// The queue-full behavior.
    pub queue_full: QueueFull,
}

/// A topology port aliasing an underlying port instance.
#[derive(Debug, Clone)]
pub struct TopologyPortInstance {
    /// The specifier defining the topology port.
    pub node: Arc<SpecTopPort>,
    /// The port instance this topology port aliases.
    pub underlying_port: Box<PortInstance>,
}

/// An FPP port instance.
///
/// The payload of each variant is its own struct, so that code which requires a
/// particular kind of port instance can say so in its types.
#[derive(Debug, Clone)]
pub enum PortInstance {
    /// A general port instance.
    General(GeneralPortInstance),
    /// A special port instance.
    Special(SpecialPortInstance),
    /// An internal port instance.
    Internal(InternalPortInstance),
    /// A topology port aliasing an underlying port instance.
    Topology(TopologyPortInstance),
}

/// The connection signature of a port instance: the parts of it that must agree
/// for one port instance to stand in for another.
#[derive(Debug, PartialEq, Eq)]
struct PortInstanceSignature {
    direction: Option<Direction>,
    array_size: i128,
    ty: Option<PortInstanceType>,
    name: String,
}

impl GeneralPortInstance {
    /// Gets the unqualified name of the port instance.
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the port instance.
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the node ID of the port instance.
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
    }

    /// Gets the size of the port array.
    pub fn get_array_size(&self) -> i128 {
        self.size
    }

    /// Gets the direction of the port instance. A general port always has one.
    pub fn get_direction(&self) -> Direction {
        match self.kind {
            GeneralKind::Output => Direction::Output,
            _ => Direction::Input,
        }
    }

    /// Gets the type of the port instance. A general port always has one.
    pub fn get_type(&self) -> PortInstanceType {
        self.ty.clone()
    }

    /// Whether this port instance is an async input.
    pub fn is_async_input(&self) -> bool {
        matches!(self.kind, GeneralKind::AsyncInput { .. })
    }

    /// Gets the node IDs of the import specifiers (if this port was imported).
    pub fn get_import_node_ids(&self) -> &[Node] {
        &self.import_node_ids
    }

    /// Gets the locations of the import specifiers (if this port was imported).
    pub fn get_import_locs(&self) -> Vec<Span> {
        get_import_locs(self.get_import_node_ids())
    }

    /// Returns a copy of this port instance imported through `import_node`.
    pub fn with_import_specifier(&self, import_node: Node) -> GeneralPortInstance {
        let mut result = self.clone();
        result.import_node_ids.push(import_node);
        result
    }

    fn signature(&self) -> PortInstanceSignature {
        PortInstanceSignature {
            direction: Some(self.get_direction()),
            array_size: self.get_array_size(),
            ty: Some(self.get_type()),
            name: self.get_unqualified_name().to_string(),
        }
    }

    /// Creates a general port instance from its specifier.
    pub fn from_spec(
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
        let size = a.get_array_size_opt(&specifier.size)?;
        // Get the priority
        let priority = a.get_big_int_value_opt(&specifier.priority);

        // Get the type
        let ty = match &specifier.port {
            Some(qid) => match a.use_def_map.get(&qid.id()) {
                Some(symbol @ Symbol::Port(_)) => PortInstanceType::DefPort(symbol.clone()),
                Some(symbol) => {
                    return Err(SemanticError::InvalidSymbol {
                        symbol_name: symbol.name().data.clone(),
                        loc: qid.span(),
                        msg: "not a port symbol".to_string(),
                        def_loc: symbol.name().span(),
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
            GeneralPortInstanceKind::Input(InputPortKind::Sync) => GeneralKind::SyncInput,
            GeneralPortInstanceKind::Output => GeneralKind::Output,
        };

        let instance = GeneralPortInstance {
            node: Arc::new(specifier.clone()),
            kind,
            size,
            ty,
            import_node_ids: vec![],
        };

        instance.check_async_input()?;
        Ok(instance)
    }

    /// Checks general async input port specifiers.
    fn check_async_input(&self) -> SemanticResult {
        if let GeneralKind::AsyncInput { .. } = self.kind
            && let PortInstanceType::DefPort(Symbol::Port(def)) = &self.ty
            && def.return_type.is_some()
        {
            return Err(SemanticError::InvalidPortInstance {
                loc: self.get_loc(),
                msg: "async input port may not return a value".to_string(),
                def_loc: def.name.span(),
            });
        }
        Ok(())
    }
}

impl SpecialPortInstance {
    /// Gets the unqualified name of the port instance.
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the port instance.
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the node ID of the port instance.
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
    }

    /// Gets the direction of the port instance. A special port always has one.
    pub fn get_direction(&self) -> Direction {
        match self.node.kind {
            SpecialPortInstanceKind::CommandRecv | SpecialPortInstanceKind::ProductRecv => {
                Direction::Input
            }
            _ => Direction::Output,
        }
    }

    /// Gets the type of the port instance. A special port always has one.
    pub fn get_type(&self) -> PortInstanceType {
        PortInstanceType::DefPort(self.symbol.clone())
    }

    /// Gets the special kind of the port instance.
    pub fn get_special_kind(&self) -> SpecialPortInstanceKind {
        self.node.kind.clone()
    }

    /// Whether this port instance is an async input.
    pub fn is_async_input(&self) -> bool {
        matches!(self.node.input_kind, Some(InputPortKind::Async))
    }

    /// Gets the node IDs of the import specifiers (if this port was imported).
    pub fn get_import_node_ids(&self) -> &[Node] {
        &self.import_node_ids
    }

    /// Gets the locations of the import specifiers (if this port was imported).
    pub fn get_import_locs(&self) -> Vec<Span> {
        get_import_locs(self.get_import_node_ids())
    }

    /// Returns a copy of this port instance imported through `import_node`.
    pub fn with_import_specifier(&self, import_node: Node) -> SpecialPortInstance {
        let mut result = self.clone();
        result.import_node_ids.push(import_node);
        result
    }

    fn signature(&self) -> PortInstanceSignature {
        PortInstanceSignature {
            direction: Some(self.get_direction()),
            array_size: 1,
            ty: Some(self.get_type()),
            name: self.get_unqualified_name().to_string(),
        }
    }

    /// Creates a special port instance from its specifier.
    pub fn from_spec(
        a: &Analysis,
        specifier: &SpecSpecialPortInstance,
    ) -> SemanticResult<SpecialPortInstance> {
        let loc = specifier.span();
        let symbol = match a.use_def_map.get(&specifier.node_id) {
            Some(symbol @ Symbol::Port(_)) => symbol.clone(),
            _ => {
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
            symbol,
            priority,
            queue_full,
            import_node_ids: vec![],
        })
    }
}

impl InternalPortInstance {
    /// Gets the unqualified name of the port instance.
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the port instance.
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the node ID of the port instance.
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
    }

    fn signature(&self) -> PortInstanceSignature {
        PortInstanceSignature {
            direction: None,
            array_size: 1,
            ty: None,
            name: self.get_unqualified_name().to_string(),
        }
    }

    /// Creates an internal port instance from its specifier.
    pub fn from_spec(
        a: &Analysis,
        specifier: &SpecInternalPort,
    ) -> SemanticResult<InternalPortInstance> {
        let loc = specifier.span();
        Analysis::check_for_duplicate_parameter(&specifier.params)?;
        if Analysis::get_num_ref_params(&specifier.params) != 0 {
            return Err(SemanticError::InvalidInternalPort {
                loc,
                msg: "internal port may not have ref parameters".to_string(),
            });
        }
        let priority = a.get_big_int_value_opt(&specifier.priority);
        Ok(InternalPortInstance {
            node: Arc::new(specifier.clone()),
            priority,
            queue_full: Analysis::get_queue_full(&specifier.queue_full),
        })
    }
}

impl TopologyPortInstance {
    /// Gets the unqualified name of the port instance.
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the port instance.
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the node ID of the port instance.
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
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

impl PortInstance {
    /// Gets the unqualified name of the port instance.
    pub fn get_unqualified_name(&self) -> &str {
        match self {
            PortInstance::General(pi) => pi.get_unqualified_name(),
            PortInstance::Special(pi) => pi.get_unqualified_name(),
            PortInstance::Internal(pi) => pi.get_unqualified_name(),
            PortInstance::Topology(pi) => pi.get_unqualified_name(),
        }
    }

    /// Gets the location of the port instance.
    pub fn get_loc(&self) -> Span {
        self.get_node_id().span()
    }

    /// Gets the node ID of the port instance.
    pub fn get_node_id(&self) -> Node {
        match self {
            PortInstance::General(pi) => pi.get_node_id(),
            PortInstance::Special(pi) => pi.get_node_id(),
            PortInstance::Internal(pi) => pi.get_node_id(),
            PortInstance::Topology(pi) => pi.get_node_id(),
        }
    }

    /// Gets the size of the port array.
    pub fn get_array_size(&self) -> i128 {
        match self {
            PortInstance::General(pi) => pi.get_array_size(),
            PortInstance::Special(_) | PortInstance::Internal(_) => 1,
            PortInstance::Topology(pi) => pi.underlying_port.get_array_size(),
        }
    }

    /// Gets the direction of the port instance.
    pub fn get_direction(&self) -> Option<Direction> {
        match self {
            PortInstance::General(pi) => Some(pi.get_direction()),
            PortInstance::Special(pi) => Some(pi.get_direction()),
            PortInstance::Internal(_) => None,
            PortInstance::Topology(pi) => pi.underlying_port.get_direction(),
        }
    }

    /// Gets the type of the port instance.
    pub fn get_type(&self) -> Option<PortInstanceType> {
        match self {
            PortInstance::General(pi) => Some(pi.get_type()),
            PortInstance::Special(pi) => Some(pi.get_type()),
            PortInstance::Internal(_) => None,
            PortInstance::Topology(pi) => pi.underlying_port.get_type(),
        }
    }

    /// Gets the special kind of the port instance, if any.
    pub fn get_special_kind(&self) -> Option<SpecialPortInstanceKind> {
        match self {
            PortInstance::Special(pi) => Some(pi.get_special_kind()),
            PortInstance::General(_) | PortInstance::Internal(_) | PortInstance::Topology(_) => {
                None
            }
        }
    }

    /// Check whether this port instance may be connected. Internal ports cannot.
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

    /// Build a topology port aliasing an underlying port instance.
    pub fn topology(node: Arc<SpecTopPort>, underlying_port: PortInstance) -> PortInstance {
        PortInstance::Topology(TopologyPortInstance {
            node,
            underlying_port: Box::new(underlying_port),
        })
    }

    /// Whether this port instance is an async input (general async, special
    /// async, or internal). Used for the passive-component check.
    pub fn is_async_input(&self) -> bool {
        match self {
            PortInstance::General(pi) => pi.is_async_input(),
            PortInstance::Special(pi) => pi.is_async_input(),
            PortInstance::Internal(_) => true,
            PortInstance::Topology(pi) => pi.underlying_port.is_async_input(),
        }
    }

    /// Gets the node IDs of the import specifiers (if this port was imported).
    /// The first item is the import of the parent interface. The final item is
    /// the import into the component. All the in-between ids are for imports
    /// into other interfaces.
    pub fn get_import_node_ids(&self) -> &[Node] {
        match self {
            PortInstance::General(pi) => pi.get_import_node_ids(),
            PortInstance::Special(pi) => pi.get_import_node_ids(),
            // Internal and topology ports cannot be imported.
            PortInstance::Internal(_) | PortInstance::Topology(_) => &[],
        }
    }

    /// Gets the locations of the import specifiers (if this port was imported).
    pub fn get_import_locs(&self) -> Vec<Span> {
        get_import_locs(self.get_import_node_ids())
    }

    pub fn with_import_specifier(&self, import_node: Node) -> PortInstance {
        match self {
            PortInstance::General(pi) => {
                PortInstance::General(pi.with_import_specifier(import_node))
            }
            PortInstance::Special(pi) => {
                PortInstance::Special(pi.with_import_specifier(import_node))
            }
            // Internal and topology ports cannot be imported.
            PortInstance::Internal(_) | PortInstance::Topology(_) => self.clone(),
        }
    }

    fn signature(&self) -> PortInstanceSignature {
        match self {
            PortInstance::General(pi) => pi.signature(),
            PortInstance::Special(pi) => pi.signature(),
            PortInstance::Internal(pi) => pi.signature(),
            PortInstance::Topology(pi) => pi.signature(),
        }
    }

    /// Whether two port instances have the same connection signature.
    pub fn signature_eq(&self, other: &PortInstance) -> bool {
        self.signature() == other.signature()
    }

    /// Creates a general port instance from its specifier.
    pub fn from_general(
        a: &Analysis,
        specifier: &SpecGeneralPortInstance,
    ) -> SemanticResult<PortInstance> {
        Ok(PortInstance::General(GeneralPortInstance::from_spec(
            a, specifier,
        )?))
    }

    /// Creates a special port instance from its specifier.
    pub fn from_special(
        a: &Analysis,
        specifier: &SpecSpecialPortInstance,
    ) -> SemanticResult<PortInstance> {
        Ok(PortInstance::Special(SpecialPortInstance::from_spec(
            a, specifier,
        )?))
    }

    /// Creates an internal port instance from its specifier.
    pub fn from_internal(
        a: &Analysis,
        specifier: &SpecInternalPort,
    ) -> SemanticResult<PortInstance> {
        Ok(PortInstance::Internal(InternalPortInstance::from_spec(
            a, specifier,
        )?))
    }
}

/// Gets the locations of a list of import specifier node IDs.
fn get_import_locs(import_node_ids: &[Node]) -> Vec<Span> {
    import_node_ids.iter().map(Spanned::span).collect()
}

// A port instance shows as its unqualified name, except for a topology port,
// which also shows the port it aliases.

impl std::fmt::Display for GeneralPortInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get_unqualified_name())
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

impl std::fmt::Display for PortInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortInstance::General(pi) => pi.fmt(f),
            PortInstance::Special(pi) => pi.fmt(f),
            PortInstance::Internal(pi) => pi.fmt(f),
            PortInstance::Topology(pi) => pi.fmt(f),
        }
    }
}

/// A set of port instances (shared by interfaces and components).
#[derive(Debug, Clone)]
pub struct PortInterface {
    /// The type of interface instance this port interface represents.
    pub instance_type: String,
    /// The map from port names to port instances.
    pub port_map: HashMap<String, PortInstance>,
    /// The map from special port kinds to special port instances.
    pub special_port_map: HashMap<SpecialPortInstanceKind, SpecialPortInstance>,
    /// The keys of `port_map`, in the order they were added.
    port_order: Vec<String>,
    /// The keys of `special_port_map`, in the order they were added.
    special_port_order: Vec<SpecialPortInstanceKind>,
}

impl PortInterface {
    pub fn new(instance_type: impl Into<String>) -> PortInterface {
        PortInterface {
            instance_type: instance_type.into(),
            port_map: HashMap::default(),
            special_port_map: HashMap::default(),
            port_order: vec![],
            special_port_order: vec![],
        }
    }

    /// The entries of the port map, in the order they were added.
    ///
    /// The port map is a hash map, whose iteration order is neither insertion
    /// order, source order, nor name order. Any check that reports only its
    /// first offending port has to impose an order of its own, or which
    /// diagnostic it produces depends on hash layout.
    ///
    /// Insertion order is not source order. A port is added when the interface
    /// that carries it is merged, so an imported port is added after the
    /// importing interface's own ports even though it is *defined* earlier —
    /// often in another file, where source order is not even well defined.
    fn ports_in_insertion_order(&self) -> impl Iterator<Item = (&String, &PortInstance)> {
        Self::in_insertion_order(&self.port_order, &self.port_map)
    }

    /// The entries of the special port map, in the order they were added. See
    /// [`PortInterface::ports_in_insertion_order`].
    fn special_ports_in_insertion_order(
        &self,
    ) -> impl Iterator<Item = (&SpecialPortInstanceKind, &SpecialPortInstance)> {
        Self::in_insertion_order(&self.special_port_order, &self.special_port_map)
    }

    /// Pair each key of `order` with its value in `map`. Every key added to one
    /// of the port maps is appended to the matching order list, and no entry is
    /// ever removed or rekeyed, so the lookup always succeeds.
    fn in_insertion_order<'a, K: std::hash::Hash + Eq, V>(
        order: &'a [K],
        map: &'a HashMap<K, V>,
    ) -> impl Iterator<Item = (&'a K, &'a V)> {
        order.iter().map(|key| {
            let value = map.get(key).expect("a port map holds all its ordered keys");
            (key, value)
        })
    }

    /// Add a port instance
    pub fn add_port_instance(&self, instance: PortInstance) -> SemanticResult<PortInterface> {
        let mut result = self.update_port_map(instance.clone())?;
        if let PortInstance::Special(special) = instance {
            result = result.update_special_port_map(special)?;
        }
        Ok(result)
    }

    /// Get a port instance by name, erroring if it is not present.
    pub fn get_port_instance(
        &self,
        name: &str,
        loc: Span,
        interface_name: &str,
    ) -> SemanticResult<PortInstance> {
        match self.port_map.get(name) {
            Some(pi) => Ok(pi.clone()),
            None => Err(SemanticError::InvalidPortInstanceId {
                loc,
                port_name: name.to_string(),
                instance_type: self.instance_type.clone(),
                interface_name: interface_name.to_string(),
            }),
        }
    }

    /// Merge in every port of `interface`, marking each as imported through
    /// `import_node`.
    ///
    /// The first port that fails to merge ends the merge, and its error is
    /// reported: which port that is depends on the order we walk `interface`'s
    /// port map, so we walk it in insertion order. See
    /// [`PortInterface::ports_in_insertion_order`].
    pub fn add_imported_interface(
        &self,
        interface: &Interface,
        import_node: Node,
    ) -> SemanticResult<PortInterface> {
        let mut result = self.clone();
        for (_, pi) in interface.port_interface.ports_in_insertion_order() {
            result = match result.add_port_instance(pi.with_import_specifier(import_node)) {
                Ok(c) => c,
                Err(err) => {
                    return Err(SemanticError::InterfaceImport {
                        loc: import_node.span(),
                        inner: Box::new(err),
                    });
                }
            };
        }
        Ok(result)
    }

    /// Add a port instance to the port map
    fn update_port_map(&self, instance: PortInstance) -> SemanticResult<PortInterface> {
        let name = instance.get_unqualified_name().to_string();
        match self.port_map.get(&name) {
            Some(prev) => Err(SemanticError::DuplicatePortInstance {
                name,
                loc: instance.get_loc(),
                import_locs: instance.get_import_locs().to_vec(),
                prev_loc: prev.get_loc(),
                prev_import_locs: prev.get_import_locs().to_vec(),
            }),
            None => {
                let mut result = self.clone();
                result.port_order.push(name.clone());
                result.port_map.insert(name, instance);
                Ok(result)
            }
        }
    }

    /// Check that `self` implements `other`: every port (general and special)
    /// in `other` exists in `self` with a matching signature.
    ///
    /// Only the first offending port is reported, so which port that is depends
    /// on the iteration order of `other`'s maps; we walk both maps in insertion
    /// order. See [`PortInterface::ports_in_insertion_order`].
    pub fn implements(&self, other: &PortInterface) -> SemanticResult {
        // Check all the ports in `other` to make sure they exist and match `self`
        for (name, pi) in other.ports_in_insertion_order() {
            match self.port_map.get(name) {
                Some(found) => {
                    // Port exists, make sure it matches theirs
                    if !found.signature_eq(pi) {
                        return Err(SemanticError::PortInterfaceInvalidPort {
                            loc: found.get_loc(),
                            def_loc: pi.get_loc(),
                        });
                    }
                }
                None => {
                    return Err(SemanticError::PortInterfaceMissingPort { loc: pi.get_loc() });
                }
            }
        }
        for (kind, pi) in other.special_ports_in_insertion_order() {
            match self.special_port_map.get(kind) {
                Some(found) => {
                    // The port exists, make sure it's the same as theirs
                    if found.signature() != pi.signature() {
                        return Err(SemanticError::PortInterfaceInvalidPort {
                            loc: found.get_loc(),
                            def_loc: pi.get_loc(),
                        });
                    }
                }
                None => {
                    return Err(SemanticError::PortInterfaceMissingPort { loc: pi.get_loc() });
                }
            }
        }
        Ok(())
    }

    /// Add a port instance to the special port map
    fn update_special_port_map(
        &self,
        instance: SpecialPortInstance,
    ) -> SemanticResult<PortInterface> {
        let kind = instance.get_special_kind();
        match self.special_port_map.get(&kind) {
            Some(prev) => Err(SemanticError::DuplicatePortInstance {
                name: kind.to_string(),
                loc: instance.get_loc(),
                import_locs: instance.get_import_locs(),
                prev_loc: prev.get_loc(),
                prev_import_locs: prev.get_import_locs(),
            }),
            None => {
                let mut result = self.clone();
                result.special_port_order.push(kind.clone());
                result.special_port_map.insert(kind, instance);
                Ok(result)
            }
        }
    }
}

/// An FPP interface.
#[derive(Debug, Clone)]
pub struct Interface {
    /// The AST node defining the interface.
    pub node: Arc<fpp_ast::DefInterface>,
    /// Imported interfaces: symbol -> (import node, import location).
    pub import_map: HashMap<Symbol, (Node, Span)>,
    /// The port interface of the component.
    pub port_interface: PortInterface,
}

impl Interface {
    pub fn new(node: Arc<fpp_ast::DefInterface>) -> Interface {
        Interface {
            node,
            import_map: HashMap::default(),
            port_interface: PortInterface::new("interface"),
        }
    }

    /// Gets the unqualified name of the interface.
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    /// Add a port instance.
    pub fn add_port_instance(&self, instance: PortInstance) -> SemanticResult<Interface> {
        let pi = self.port_interface.add_port_instance(instance)?;
        let mut result = self.clone();
        result.port_interface = pi;
        Ok(result)
    }

    /// Merge in every port of `interface`. On failure `self` is left unchanged,
    /// discarding the ports already merged from `interface`.
    pub fn add_imported_interface(
        &mut self,
        interface: &Interface,
        import_node: Node,
    ) -> SemanticResult {
        self.port_interface = self
            .port_interface
            .add_imported_interface(interface, import_node)?;
        Ok(())
    }

    pub fn add_imported_interface_symbol(
        &self,
        symbol: Symbol,
        import: &SpecInterfaceImport,
    ) -> SemanticResult<Interface> {
        if let Some((_, prev_loc)) = self.import_map.get(&symbol) {
            return Err(SemanticError::DuplicateInterface {
                name: symbol.name().data.clone(),
                loc: import.span(),
                prev_loc: *prev_loc,
            });
        }
        let mut result = self.clone();
        result
            .import_map
            .insert(symbol, (import.node_id, import.span()));
        Ok(result)
    }

    /// The interfaces imported by this one, as (symbol, import specifier node)
    /// pairs in source order.
    pub fn imports_in_source_order(&self) -> Vec<(Symbol, Node)> {
        let mut imports: Vec<(Symbol, Node)> = self
            .import_map
            .iter()
            .map(|(symbol, (node, _))| (symbol.clone(), *node))
            .collect();
        imports.sort_by(|x, y| cmp_span(&x.1.span(), &y.1.span()));
        imports
    }
}

/// Resolve `interface` in place by merging in the interfaces it imports.
///
/// The first import that fails to merge ends the merge: exactly one error is
/// reported per interface, and it is the error of the first offending import in
/// source order.
///
/// On failure the imports merged before the offending one stay merged into
/// `interface`. Checking continues after the error, so dropping the successful
/// imports would delete ports the user did write and draw a second round of
/// errors against them.
///
/// `interface_map` normally holds every interface that `interface` imports. An
/// entry is absent only when the imported interface itself failed to resolve,
/// and that failure has already been reported, so we skip it rather than panic.
pub fn resolve_interface(
    interface_map: &HashMap<Symbol, Interface>,
    interface: &mut Interface,
) -> SemanticResult {
    for (symbol, node) in interface.imports_in_source_order() {
        if let Some(imported) = interface_map.get(&symbol) {
            let imported = imported.clone();
            interface.add_imported_interface(&imported, node)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Direction, PortInstance, PortInterface};
    use crate::semantics::{Component, SymbolInterface, Topology};
    use crate::{Analysis, add_state_enums, check_semantics};
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

    /// A port interface remembers the order its ports were added in, so the
    /// checks that walk it report the same port however many ports there are
    /// and however the hash map happens to lay them out.
    #[test]
    fn ports_are_walked_in_insertion_order() {
        with_analysis(|component, _| {
            let pi = &component.port_interface;
            let names: Vec<&str> = pi
                .ports_in_insertion_order()
                .map(|(name, _)| name.as_str())
                .collect();
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
            let kinds: Vec<SpecialPortInstanceKind> = pi
                .special_ports_in_insertion_order()
                .map(|(kind, _)| kind.clone())
                .collect();
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

    /// `Interface::imports_in_source_order` orders the imports by the location
    /// of the import specifier, not by the iteration order of the hash map, so
    /// `resolve_interface` merges them in source order.
    #[test]
    fn imports_are_merged_in_source_order() {
        const IMPORTS: &str = r#"
interface A {}
interface B {}
interface C {}
interface D {}

interface I {
  import D
  import C
  import B
  import A
}
"#;
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let source = SourceFile::new("interface_imports.fpp", IMPORTS.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            add_state_enums(&mut ast);
            let _ = check_semantics(&mut a, vec![&ast]);
            let interface = a
                .interface_map
                .values()
                .find(|i| i.get_unqualified_name() == "I")
                .expect("interface I was resolved");
            let imports = interface.imports_in_source_order();
            let names: Vec<&str> = imports
                .iter()
                .map(|(symbol, _)| symbol.name().data.as_str())
                .collect();
            assert_eq!(names, vec!["D", "C", "B", "A"]);
            // The import specifier nodes are in source order too.
            let starts: Vec<_> = imports
                .iter()
                .map(|(_, node)| node.span().start().line())
                .collect();
            assert!(starts.windows(2).all(|w| w[0] < w[1]));
        });
        let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
        assert_eq!(output, "", "expected no diagnostics");
    }
}
