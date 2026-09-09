use std::collections::VecDeque;
use std::fmt::{Debug, Display, Formatter, Write};

/// A qualified or unqualified name
#[derive(Clone)]
pub struct QualifiedName {
    qualifier: VecDeque<String>,
    base: String,
}

impl QualifiedName {
    /// Convert a qualified name to an identifier list
    pub fn to_ident_list(&self) -> VecDeque<String> {
        let mut out = self.qualifier.clone();
        out.push_back(self.base.clone());
        out
    }
}

/// Create a qualified name from an identifier
impl From<String> for QualifiedName {
    fn from(value: String) -> Self {
        QualifiedName {
            qualifier: VecDeque::new(),
            base: value,
        }
    }
}

impl From<Vec<String>> for QualifiedName {
    fn from(value: Vec<String>) -> Self {
        let inter: VecDeque<String> = value.into();
        inter.into()
    }
}

/// Create a qualified name A.B.C from an identifier list [ A, B, C ]
impl From<VecDeque<String>> for QualifiedName {
    fn from(mut value: VecDeque<String>) -> Self {
        let base = value
            .pop_back()
            .expect("qualified name must have at least one token");
        QualifiedName {
            base,
            qualifier: value,
        }
    }
}

/// Create a qualified name from a qualified identifier
impl From<&fpp_ast::QualIdent> for QualifiedName {
    fn from(value: &fpp_ast::QualIdent) -> Self {
        fn to_qualifier(value: &fpp_ast::QualIdent, mut q: VecDeque<String>) -> VecDeque<String> {
            match value {
                fpp_ast::QualIdent::Unqualified(ident) => {
                    q.push_front(ident.data.clone());
                    q
                }
                fpp_ast::QualIdent::Qualified(fpp_ast::Qualified {
                    qualifier, name, ..
                }) => {
                    q.push_back(name.data.clone());
                    to_qualifier(qualifier, q)
                }
            }
        }

        match value {
            fpp_ast::QualIdent::Qualified(fpp_ast::Qualified {
                qualifier, name, ..
            }) => Self {
                qualifier: to_qualifier(qualifier, VecDeque::new()),
                base: name.data.clone(),
            },
            fpp_ast::QualIdent::Unqualified(name) => Self {
                qualifier: VecDeque::new(),
                base: name.data.clone(),
            },
        }
    }
}

impl Debug for QualifiedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let v: Vec<String> = self.qualifier.clone().into();
        f.write_str(&v.join("."))?;
        f.write_char('.')?;
        f.write_str(&self.base)
    }
}

/// Convert a qualified name to a string
impl Display for QualifiedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let v: Vec<String> = self.to_ident_list().into();
        f.write_str(&v.join("."))
    }
}
