use crate::Analysis;
use crate::errors::{SemanticResult, annotate_emitted_errors, annotate_result};
use crate::semantics::QualifiedName;
use fpp_core::Spanned;

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
    /// The fully-qualified name of the implied use
    pub fn name(&self) -> &QualifiedName {
        &self.name
    }

    /// The AST node id associated with the implied use
    pub fn id(&self) -> fpp_core::Node {
        self.id
    }

    /// Optional annotations for error reporting
    pub fn annotations(&self) -> &[String] {
        &self.annotations
    }

    /// The implied use as an expression. The name is absolute, and every node of
    /// the expression has the id of the implied use.
    pub fn as_expr_node(&self) -> fpp_ast::Expr {
        self.as_expr_node_with(|node| node, true)
    }

    pub fn as_unique_expr_node(&self) -> fpp_ast::Expr {
        self.as_expr_node_with(ImpliedUse::replicate_id, false)
    }

    /// The implied use as an absolute qualified identifier.
    ///
    /// Only the outermost node has the id of the implied use; each qualifier
    /// and identifier gets a new id at the same location. A use records its
    /// definition under the id of each level it resolves, so the entry under
    /// the id of the implied use is always the symbol of the full name.
    pub fn as_qual_ident_node(&self) -> fpp_ast::QualIdent {
        let node_list = self
            .name
            .to_ident_list()
            .into_iter()
            .map(|data| fpp_ast::Ident {
                node_id: ImpliedUse::replicate_id(self.id),
                data,
            })
            .collect();
        let mut qual_ident = fpp_ast::QualIdent::from_node_list(node_list, true);
        match &mut qual_ident {
            fpp_ast::QualIdent::Unqualified(unqualified) => unqualified.node_id = self.id,
            fpp_ast::QualIdent::Qualified(qualified) => qualified.node_id = self.id,
        }
        qual_ident
    }

    pub fn as_type_name_node(&self) -> fpp_ast::TypeName {
        fpp_ast::TypeName {
            kind: fpp_ast::TypeNameKind::QualIdent(self.as_qual_ident_node()),
            node_id: self.id,
        }
    }

    /// Annotates an error result with the notes of this implied use
    pub fn annotate_result<T>(&self, r: SemanticResult<T>) -> SemanticResult<T> {
        annotate_result(r, &self.notes())
    }

    /// Runs `f`, annotating every error that `f` emits with the notes of this
    /// implied use. See [`annotate_emitted_errors`].
    pub fn annotate_emitted_errors<T>(&self, f: impl FnOnce() -> T) -> T {
        annotate_emitted_errors(self.notes(), f)
    }

    /// The implied use as an expression whose nodes get their ids from `id`
    fn as_expr_node_with(
        &self,
        id: fn(fpp_core::Node) -> fpp_core::Node,
        is_absolute: bool,
    ) -> fpp_ast::Expr {
        let mut tail = self.name.to_ident_list();
        let head = tail.pop_front().unwrap();
        tail.into_iter().fold(
            fpp_ast::Expr {
                node_id: id(self.id),
                kind: fpp_ast::ExprKind::Ident {
                    value: head,
                    is_absolute,
                },
            },
            |e1, s| fpp_ast::Expr {
                node_id: id(self.id),
                kind: fpp_ast::ExprKind::Dot {
                    e: Box::new(e1),
                    id: fpp_ast::Ident {
                        node_id: id(self.id),
                        data: s,
                    },
                },
            },
        )
    }

    /// The notes for an error at the point of this implied use
    fn notes(&self) -> Vec<String> {
        let mut notes = vec![format!(
            "the symbol {} has an implied use at the point of the error",
            self.name
        )];
        notes.extend(self.annotations.iter().cloned());
        notes
    }
}

/// The kind of an implied use
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ImpliedUseKind {
    Constant,
    Port,
    Type,
}

/// The implied uses associated with a single AST node, grouped by kind
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

impl ImpliedUse {
    /// The qualified names of implied type uses.
    /// Each name is a list of identifiers
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

    /// The qualified names of implied constant uses.
    /// Each name is a list of identifiers
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
        ident_list: Vec<String>,
        id: fpp_core::Node,
        annotations: Vec<String>,
    ) -> ImpliedUse {
        ImpliedUse::from_name_and_id(ident_list.into(), id, annotations)
    }
}
