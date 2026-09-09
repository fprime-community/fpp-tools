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

/// An implied use of an FPP symbol
#[derive(Clone, Debug)]
pub struct ImpliedUse {
    /// The fully-qualified name of the implied use
    name: QualifiedName,
    /// The AST node id associated with the implied use
    id: fpp_core::Node,
}

impl ImpliedUse {
    pub fn new(name: QualifiedName, id: fpp_core::Node) -> ImpliedUse {
        ImpliedUse { name, id }
    }

    /// Construct an implied use from an identifier list, replicating the node id
    /// so the implied use has its own stable, distinct node id
    pub fn from_ident_list_and_id(idents: Vec<String>, id: fpp_core::Node) -> ImpliedUse {
        ImpliedUse {
            name: idents.into(),
            id: ImpliedUse::replicate_node_id(id),
        }
    }

    pub fn id(&self) -> fpp_core::Node {
        self.id
    }

    pub fn name(&self) -> &QualifiedName {
        &self.name
    }

    /// Create a new ID at the same location as id
    fn replicate_node_id(id: fpp_core::Node) -> fpp_core::Node {
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
        self.as_expr_impl(ImpliedUse::replicate_node_id)
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
