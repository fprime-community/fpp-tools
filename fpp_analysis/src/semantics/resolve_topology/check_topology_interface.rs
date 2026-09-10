use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::Topology;
use fpp_ast::AstNode;
use fpp_core::Spanned;

/// Check a topology implements all the interfaces that are listed in the AST
pub fn check(a: &Analysis, t: &Topology) -> SemanticResult {
    for impl_use in t.implements() {
        let Some(iface) = a.get_interface(impl_use.id()) else {
            continue;
        };
        match t.port_interface.implements(&iface.port_interface) {
            Ok(_) => {}
            Err(err) => {
                return Err(SemanticError::InterfaceImplements {
                    loc: impl_use.span(),
                    inner: Box::new(err),
                });
            }
        }
    }
    Ok(())
}
