use crate::Analysis;
use crate::semantics::QualifiedName;
use fpp_core::Spanned;

/// The kind of an implied use
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ImpliedUseKind {
    Constant,
    Port,
    Type,
}

/// The set of implied uses associated with a single AST node, grouped by kind
#[derive(Clone, Default, Debug)]
pub struct ImpliedUseSet {
    pub constants: Vec<ImpliedUse>,
    pub ports: Vec<ImpliedUse>,
    pub types: Vec<ImpliedUse>,
}

impl ImpliedUseSet {
    /// Gets the implied uses of the given kind
    pub fn get(&self, kind: ImpliedUseKind) -> &[ImpliedUse] {
        match kind {
            ImpliedUseKind::Constant => &self.constants,
            ImpliedUseKind::Port => &self.ports,
            ImpliedUseKind::Type => &self.types,
        }
    }

    /// Adds an implied use of the given kind
    pub fn add(&mut self, kind: ImpliedUseKind, iu: ImpliedUse) {
        match kind {
            ImpliedUseKind::Constant => self.constants.push(iu),
            ImpliedUseKind::Port => self.ports.push(iu),
            ImpliedUseKind::Type => self.types.push(iu),
        }
    }
}

/// An implied use of an FPP symbol
#[derive(Clone, Debug)]
pub struct ImpliedUse {
    /// The fully-qualified name of the implied use
    name: QualifiedName,
    /// The AST node id associated with the implied use
    id: fpp_core::Node,
    /// Optional annotations for error reporting
    annotations: Vec<String>,
}

impl ImpliedUse {
    pub fn new(name: QualifiedName, id: fpp_core::Node) -> ImpliedUse {
        ImpliedUse {
            name,
            id,
            annotations: Vec::new(),
        }
    }

    /// Construct an implied use from a name and a node id
    pub fn from_name_and_id(
        name: QualifiedName,
        id: fpp_core::Node,
        annotations: Vec<String>,
    ) -> ImpliedUse {
        ImpliedUse {
            name,
            id,
            annotations,
        }
    }

    /// Construct an implied use from an identifier list and a node id
    pub fn from_ident_list_and_id(
        idents: Vec<String>,
        id: fpp_core::Node,
        annotations: Vec<String>,
    ) -> ImpliedUse {
        ImpliedUse::from_name_and_id(idents.into(), id, annotations)
    }

    pub fn id(&self) -> fpp_core::Node {
        self.id
    }

    pub fn name(&self) -> &QualifiedName {
        &self.name
    }

    pub fn annotations(&self) -> &[String] {
        &self.annotations
    }

    /// The qualified names of implied type uses. Each name is a list of
    /// identifiers.
    pub fn get_topology_types(a: &Analysis) -> Vec<Vec<String>> {
        if !a.dictionary_generation {
            return Vec::new();
        }
        let mut out = vec![
            vec![
                "Fw".to_string(),
                "DpCfg".to_string(),
                "ProcType".to_string(),
            ],
            vec!["Fw".to_string(), "DpState".to_string()],
        ];
        out.extend(
            [
                "FwChanIdType",
                "FwDpIdType",
                "FwDpPriorityType",
                "FwEventIdType",
                "FwOpcodeType",
                "FwPacketDescriptorType",
                "FwSizeType",
                "FwSizeStoreType",
                "FwTimeBaseStoreType",
                "FwTimeContextStoreType",
                "FwTlmPacketizeIdType",
            ]
            .into_iter()
            .map(|name| vec![name.to_string()]),
        );
        out
    }

    /// The qualified names of implied constant uses. Each name is a list of
    /// identifiers.
    pub fn get_topology_constants(a: &Analysis) -> Vec<Vec<String>> {
        if !a.dictionary_generation {
            return Vec::new();
        }
        vec![
            vec![
                "Fw".to_string(),
                "DpCfg".to_string(),
                "CONTAINER_USER_DATA_SIZE".to_string(),
            ],
            vec!["FW_FIXED_LENGTH_STRING_SIZE".to_string()],
        ]
    }

    /// Create a new ID at the same location as id
    pub fn replicate_id(id: fpp_core::Node) -> fpp_core::Node {
        fpp_core::Node::new(id.span())
    }

    fn as_expr_impl(&self, pred: fn(fpp_core::Node) -> fpp_core::Node) -> fpp_ast::Expr {
        let mut tail = self.name.to_ident_list();
        let head = tail.pop_front().unwrap();
        tail.into_iter().fold(
            fpp_ast::Expr {
                node_id: pred(self.id),
                kind: fpp_ast::ExprKind::Ident(head),
            },
            |e1, s| fpp_ast::Expr {
                node_id: pred(self.id),
                kind: fpp_ast::ExprKind::Dot {
                    e: Box::new(e1),
                    id: fpp_ast::Ident {
                        node_id: pred(self.id),
                        data: s,
                    },
                },
            },
        )
    }

    pub fn as_expr(&self) -> fpp_ast::Expr {
        self.as_expr_impl(|node| node)
    }

    pub fn as_unique_expr(&self) -> fpp_ast::Expr {
        self.as_expr_impl(ImpliedUse::replicate_id)
    }

    pub fn as_qual_ident(&self) -> fpp_ast::QualIdent {
        let mut tail = self.name.to_ident_list();
        let head = tail.pop_front().unwrap();
        tail.into_iter().fold(
            fpp_ast::QualIdent::Unqualified(fpp_ast::Ident {
                data: head,
                node_id: self.id,
            }),
            |e1: fpp_ast::QualIdent, s| {
                fpp_ast::QualIdent::Qualified(fpp_ast::Qualified {
                    qualifier: Box::new(e1),
                    name: fpp_ast::Ident {
                        data: s,
                        node_id: self.id,
                    },
                    node_id: self.id,
                })
            },
        )
    }

    pub fn as_type_name(&self) -> fpp_ast::TypeName {
        fpp_ast::TypeName {
            kind: fpp_ast::TypeNameKind::QualIdent(self.as_qual_ident()),
            node_id: self.id,
        }
    }
}
