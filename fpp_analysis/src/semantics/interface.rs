use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Symbol, SymbolInterface};
use fpp_ast::{
    AstNode, GeneralPortInstanceKind, InputPortKind, QueueFull, SpecGeneralPortInstance,
    SpecInterfaceImport, SpecInternalPort, SpecSpecialPortInstance, SpecialPortInstanceKind,
};
use fpp_core::{Node, Span, Spanned};
use rustc_hash::FxHashMap as HashMap;

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

/// An FPP port instance.
#[derive(Debug, Clone)]
pub enum PortInstance {
    /// A general port instance.
    General {
        node_id: Node,
        loc: Span,
        name: String,
        kind: GeneralKind,
        size: i128,
        ty: PortInstanceType,
        import_locs: Vec<Span>,
    },
    /// A special port instance.
    Special {
        node_id: Node,
        loc: Span,
        name: String,
        kind: SpecialPortInstanceKind,
        input_kind: Option<InputPortKind>,
        symbol: Symbol,
        priority: Option<i128>,
        queue_full: Option<QueueFull>,
        import_locs: Vec<Span>,
    },
    Internal {
        node_id: Node,
        loc: Span,
        name: String,
        priority: Option<i128>,
        queue_full: QueueFull,
        import_locs: Vec<Span>,
    },
    /// A topology port aliasing an underlying port instance.
    Topology {
        node_id: Node,
        loc: Span,
        name: String,
        underlying: Box<PortInstance>,
    },
}

impl PortInstance {
    /// Gets the unqualified name of the port instance.
    pub fn get_unqualified_name(&self) -> &str {
        match self {
            PortInstance::General { name, .. }
            | PortInstance::Special { name, .. }
            | PortInstance::Internal { name, .. }
            | PortInstance::Topology { name, .. } => name,
        }
    }

    /// Gets the location of the port instance.
    pub fn get_loc(&self) -> Span {
        match self {
            PortInstance::General { loc, .. }
            | PortInstance::Special { loc, .. }
            | PortInstance::Internal { loc, .. }
            | PortInstance::Topology { loc, .. } => *loc,
        }
    }

    /// Gets the node ID of the port instance.
    pub fn get_node_id(&self) -> Node {
        match self {
            PortInstance::General { node_id, .. }
            | PortInstance::Special { node_id, .. }
            | PortInstance::Internal { node_id, .. }
            | PortInstance::Topology { node_id, .. } => *node_id,
        }
    }

    /// Gets the size of the port array.
    pub fn get_array_size(&self) -> i128 {
        match self {
            PortInstance::General { size, .. } => *size,
            PortInstance::Special { .. } | PortInstance::Internal { .. } => 1,
            PortInstance::Topology { underlying, .. } => underlying.get_array_size(),
        }
    }

    /// Gets the direction of the port instance.
    pub fn get_direction(&self) -> Option<Direction> {
        match self {
            PortInstance::General { kind, .. } => Some(match kind {
                GeneralKind::Output => Direction::Output,
                _ => Direction::Input,
            }),
            PortInstance::Special { kind, .. } => Some(match kind {
                SpecialPortInstanceKind::CommandRecv | SpecialPortInstanceKind::ProductRecv => {
                    Direction::Input
                }
                _ => Direction::Output,
            }),
            PortInstance::Internal { .. } => None,
            PortInstance::Topology { underlying, .. } => underlying.get_direction(),
        }
    }

    /// Gets the type of the port instance.
    pub fn get_type(&self) -> Option<PortInstanceType> {
        match self {
            PortInstance::General { ty, .. } => Some(ty.clone()),
            PortInstance::Special { symbol, .. } => Some(PortInstanceType::DefPort(symbol.clone())),
            PortInstance::Internal { .. } => None,
            PortInstance::Topology { underlying, .. } => underlying.get_type(),
        }
    }

    /// Gets the special kind of the port instance, if any.
    pub fn get_special_kind(&self) -> Option<SpecialPortInstanceKind> {
        match self {
            PortInstance::Special { kind, .. } => Some(kind.clone()),
            PortInstance::General { .. }
            | PortInstance::Internal { .. }
            | PortInstance::Topology { .. } => None,
        }
    }

    /// The general-port kind, for a general port instance (else `None`).
    pub fn general_kind(&self) -> Option<GeneralKind> {
        match self {
            PortInstance::General { kind, .. } => Some(kind.clone()),
            _ => None,
        }
    }

    /// The async-input priority of a general port instance, if any.
    pub fn priority(&self) -> Option<i128> {
        match self {
            PortInstance::General {
                kind: GeneralKind::AsyncInput { priority, .. },
                ..
            } => *priority,
            _ => None,
        }
    }

    /// The async-input queue-full behavior of a general port instance, if any.
    pub fn queue_full(&self) -> Option<QueueFull> {
        match self {
            PortInstance::General {
                kind: GeneralKind::AsyncInput { queue_full, .. },
                ..
            } => Some(queue_full.clone()),
            _ => None,
        }
    }

    /// Whether this port instance is serial (untyped).
    pub fn is_serial(&self) -> bool {
        matches!(self.get_type(), Some(PortInstanceType::Serial))
    }

    /// The port-definition symbol for a typed port, if any (else serial/none).
    pub fn type_symbol(&self) -> Option<Symbol> {
        match self.get_type() {
            Some(PortInstanceType::DefPort(sym)) => Some(sym),
            _ => None,
        }
    }

    /// Check whether this port instance may be connected. Internal ports cannot.
    pub fn require_connection_at(&self, loc: Span) -> SemanticResult {
        match self {
            PortInstance::Internal { .. } => Err(SemanticError::InvalidPortKind {
                loc,
                msg: "cannot connect to internal port".to_string(),
                spec_loc: self.get_loc(),
            }),
            _ => Ok(()),
        }
    }

    /// Build a topology port aliasing an underlying port instance.
    pub fn topology(
        node_id: Node,
        loc: Span,
        name: String,
        underlying: PortInstance,
    ) -> PortInstance {
        PortInstance::Topology {
            node_id,
            loc,
            name,
            underlying: Box::new(underlying),
        }
    }

    /// Whether this port instance is an async input (general async, special
    /// async, or internal). Used for the passive-component check.
    pub fn is_async_input(&self) -> bool {
        match self {
            PortInstance::General {
                kind: GeneralKind::AsyncInput { .. },
                ..
            } => true,
            PortInstance::Special { input_kind, .. } => {
                matches!(input_kind, Some(InputPortKind::Async))
            }
            PortInstance::Internal { .. } => true,
            PortInstance::General { .. } => false,
            PortInstance::Topology { underlying, .. } => underlying.is_async_input(),
        }
    }

    /// Gets the locations of the import specifiers (if this port was imported).
    /// The first item is the import of the parent interface. The final item is
    /// the import into the component. All the in-between locs are for imports
    /// into other interfaces.
    pub fn get_import_locs(&self) -> &[Span] {
        match self {
            PortInstance::General { import_locs, .. }
            | PortInstance::Special { import_locs, .. }
            | PortInstance::Internal { import_locs, .. } => import_locs,
            PortInstance::Topology { .. } => &[],
        }
    }

    pub fn with_import_specifier(&self, import_loc: Span) -> PortInstance {
        let mut clone = self.clone();
        match &mut clone {
            PortInstance::General { import_locs, .. }
            | PortInstance::Special { import_locs, .. }
            | PortInstance::Internal { import_locs, .. } => import_locs.push(import_loc),
            // Topology ports cannot be imported.
            PortInstance::Topology { .. } => {}
        }
        clone
    }

    /// Whether two port instances have the same connection signature.
    pub fn signature_eq(&self, other: &PortInstance) -> bool {
        self.get_direction() == other.get_direction()
            && self.get_array_size() == other.get_array_size()
            && self.get_type() == other.get_type()
            && self.get_unqualified_name() == other.get_unqualified_name()
    }

    /// Creates a general port instance from its specifier.
    pub fn from_general(
        a: &Analysis,
        specifier: &SpecGeneralPortInstance,
    ) -> SemanticResult<PortInstance> {
        let node_id = specifier.node_id;
        let loc = specifier.span();

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
            if specifier.queue_full.is_some() {
                return Err(SemanticError::InvalidQueueFull { loc });
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
                queue_full: specifier.queue_full.clone().unwrap_or(QueueFull::Assert),
            },
            GeneralPortInstanceKind::Input(InputPortKind::Guarded) => GeneralKind::GuardedInput,
            GeneralPortInstanceKind::Input(InputPortKind::Sync) => GeneralKind::SyncInput,
            GeneralPortInstanceKind::Output => GeneralKind::Output,
        };

        let instance = PortInstance::General {
            node_id,
            loc,
            name: specifier.name.data.clone(),
            kind,
            size,
            ty,
            import_locs: vec![],
        };

        check_general_async_input(&instance)?;
        Ok(instance)
    }

    /// Creates a special port instance from its specifier.
    pub fn from_special(
        a: &Analysis,
        specifier: &SpecSpecialPortInstance,
    ) -> SemanticResult<PortInstance> {
        let node_id = specifier.node_id;
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
            if specifier.queue_full.is_some() {
                return Err(SemanticError::InvalidQueueFull { loc });
            }
        }

        // Get the priority
        let priority = a.get_big_int_value_opt(&specifier.priority);
        let queue_full = match specifier.kind {
            SpecialPortInstanceKind::ProductRecv => {
                Some(specifier.queue_full.clone().unwrap_or(QueueFull::Assert))
            }
            _ => None,
        };

        Ok(PortInstance::Special {
            node_id,
            loc,
            name: specifier.name.data.clone(),
            kind: specifier.kind.clone(),
            input_kind: specifier.input_kind.clone(),
            symbol,
            priority,
            queue_full,
            import_locs: vec![],
        })
    }

    /// Creates an internal port instance from its specifier.
    pub fn from_internal(
        a: &Analysis,
        specifier: &SpecInternalPort,
    ) -> SemanticResult<PortInstance> {
        let loc = specifier.span();
        Analysis::check_for_duplicate_parameter(&specifier.params)?;
        if Analysis::get_num_ref_params(&specifier.params) != 0 {
            return Err(SemanticError::InvalidInternalPort {
                loc,
                msg: "internal port may not have ref parameters".to_string(),
            });
        }
        let priority = a.get_big_int_value_opt(&specifier.priority);
        Ok(PortInstance::Internal {
            node_id: specifier.node_id,
            loc,
            name: specifier.name.data.clone(),
            priority,
            queue_full: Analysis::get_queue_full(&specifier.queue_full),
            import_locs: vec![],
        })
    }
}

/// Checks general async input port specifiers.
fn check_general_async_input(instance: &PortInstance) -> SemanticResult {
    if let PortInstance::General {
        loc,
        kind: GeneralKind::AsyncInput { .. },
        ty: PortInstanceType::DefPort(Symbol::Port(def)),
        ..
    } = instance
        && def.return_type.is_some()
    {
        return Err(SemanticError::InvalidPortInstance {
            loc: *loc,
            msg: "async input port may not return a value".to_string(),
            def_loc: def.name.span(),
        });
    }
    Ok(())
}

/// A set of port instances (shared by interfaces and components).
#[derive(Debug, Clone)]
pub struct PortInterface {
    /// The type of interface instance this port interface represents.
    pub instance_type: String,
    /// The map from port names to port instances.
    pub port_map: HashMap<String, PortInstance>,
    /// The map from special port kinds to special port instances.
    pub special_port_map: HashMap<String, PortInstance>,
}

impl PortInterface {
    pub fn new(instance_type: impl Into<String>) -> PortInterface {
        PortInterface {
            instance_type: instance_type.into(),
            port_map: HashMap::default(),
            special_port_map: HashMap::default(),
        }
    }

    /// Add a port instance
    pub fn add_port_instance(&self, instance: PortInstance) -> SemanticResult<PortInterface> {
        let mut result = self.update_port_map(instance.clone())?;
        if let Some(kind) = instance.get_special_kind() {
            result = result.update_special_port_map(&kind, instance)?;
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

    pub fn add_imported_interface(
        &self,
        interface: &Interface,
        import_loc: Span,
    ) -> SemanticResult<PortInterface> {
        let mut result = self.clone();
        for pi in interface.port_interface.port_map.values() {
            result = match result.add_port_instance(pi.with_import_specifier(import_loc)) {
                Ok(c) => c,
                Err(err) => {
                    return Err(SemanticError::InterfaceImport {
                        loc: import_loc,
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
                result.port_map.insert(name, instance);
                Ok(result)
            }
        }
    }

    /// Check that `self` implements `other`: every port (general and special)
    /// in `other` exists in `self` with a matching signature.
    pub fn implements(&self, other: &PortInterface) -> SemanticResult {
        // Check all the ports in `other` to make sure they exist and match `self`
        for (name, pi) in &other.port_map {
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
        for (kind, pi) in &other.special_port_map {
            match self.special_port_map.get(kind) {
                Some(found) => {
                    // The port exists, make sure it's the same as theirs
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
        Ok(())
    }

    /// Add a port instance to the special port map
    fn update_special_port_map(
        &self,
        kind: &SpecialPortInstanceKind,
        instance: PortInstance,
    ) -> SemanticResult<PortInterface> {
        let key = format!("{:?}", kind);
        match self.special_port_map.get(&key) {
            Some(prev) => Err(SemanticError::DuplicatePortInstance {
                name: key,
                loc: instance.get_loc(),
                import_locs: instance.get_import_locs().to_vec(),
                prev_loc: prev.get_loc(),
                prev_import_locs: prev.get_import_locs().to_vec(),
            }),
            None => {
                let mut result = self.clone();
                result.special_port_map.insert(key, instance);
                Ok(result)
            }
        }
    }
}

/// An FPP interface.
#[derive(Debug, Clone)]
pub struct Interface {
    /// The symbol defining the interface.
    pub symbol: Symbol,
    /// Imported interfaces: symbol -> (import node, import location).
    pub import_map: HashMap<Symbol, (Node, Span)>,
    /// The port interface of the component.
    pub port_interface: PortInterface,
}

impl Interface {
    pub fn new(symbol: Symbol) -> Interface {
        Interface {
            symbol,
            import_map: HashMap::default(),
            port_interface: PortInterface::new("interface"),
        }
    }

    /// Add a port instance.
    pub fn add_port_instance(&self, instance: PortInstance) -> SemanticResult<Interface> {
        let pi = self.port_interface.add_port_instance(instance)?;
        let mut result = self.clone();
        result.port_interface = pi;
        Ok(result)
    }

    pub fn add_imported_interface(
        &self,
        interface: &Interface,
        import_loc: Span,
    ) -> SemanticResult<Interface> {
        let pi = self
            .port_interface
            .add_imported_interface(interface, import_loc)?;
        let mut result = self.clone();
        result.port_interface = pi;
        Ok(result)
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
}

/// Resolve an interface by transitively merging its imported interfaces.
pub fn resolve_interface(
    interface_map: &HashMap<Symbol, Interface>,
    interface: &Interface,
) -> SemanticResult<Interface> {
    let mut result = interface.clone();
    let imports: Vec<(Symbol, (Node, Span))> = interface
        .import_map
        .iter()
        .map(|(s, v)| (s.clone(), *v))
        .collect();
    for (symbol, (_node, loc)) in imports {
        if let Some(imported) = interface_map.get(&symbol) {
            let imported = imported.clone();
            result = result.add_imported_interface(&imported, loc)?;
        }
    }
    Ok(result)
}
