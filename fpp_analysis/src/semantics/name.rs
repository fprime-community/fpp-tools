use std::collections::VecDeque;
use std::fmt::{Debug, Display, Formatter};

/// An unqualified name
pub type Unqualified = String;

/// A qualified or unqualified name
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QualifiedName {
    qualifier: VecDeque<Unqualified>,
    base: Unqualified,
}

impl QualifiedName {
    /// Create a qualified name A.B.C from an identifier list [ A, B, C ]
    pub fn from_ident_list(il: Vec<Unqualified>) -> QualifiedName {
        let mut qualifier: VecDeque<Unqualified> = il.into();
        let base = qualifier.pop_back().expect("empty identifier list");
        QualifiedName { qualifier, base }
    }

    /// Create a qualified name from an identifier
    pub fn from_ident(id: Unqualified) -> QualifiedName {
        QualifiedName {
            qualifier: VecDeque::new(),
            base: id,
        }
    }

    /// Create a qualified name from a qualified identifier
    pub fn from_qual_ident(qual_ident: &fpp_ast::QualIdent) -> QualifiedName {
        QualifiedName::from_ident_list(qual_ident.to_ident_list())
    }

    /// The qualifier of the name
    pub fn qualifier(&self) -> &VecDeque<String> {
        &self.qualifier
    }

    /// The base of the name
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Convert a qualified name to an identifier list
    pub fn to_ident_list(&self) -> VecDeque<String> {
        let mut out = self.qualifier.clone();
        out.push_back(self.base.clone());
        out
    }

    /// Computes a short qualified name. Deletes the longest prefix provided by
    /// the enclosing scope.
    ///
    /// The base of the name is never deleted: when the enclosing scope provides
    /// the whole name, the short name is the base.
    pub fn short_name(&self, enclosing_names: &[String]) -> QualifiedName {
        let ident_list = self.to_ident_list();
        let mut skip = 0;
        while skip < enclosing_names.len()
            && skip + 1 < ident_list.len()
            && enclosing_names[skip] == ident_list[skip]
        {
            skip += 1;
        }
        ident_list
            .into_iter()
            .skip(skip)
            .collect::<VecDeque<String>>()
            .into()
    }
}

/// Create a qualified name from an identifier
impl From<String> for QualifiedName {
    fn from(value: String) -> Self {
        QualifiedName::from_ident(value)
    }
}

/// Create a qualified name A.B.C from an identifier list [ A, B, C ]
impl From<Vec<String>> for QualifiedName {
    fn from(value: Vec<String>) -> Self {
        QualifiedName::from_ident_list(value)
    }
}

/// Create a qualified name A.B.C from an identifier list [ A, B, C ]
impl From<VecDeque<String>> for QualifiedName {
    fn from(mut value: VecDeque<String>) -> Self {
        let base = value.pop_back().expect("empty identifier list");
        QualifiedName {
            base,
            qualifier: value,
        }
    }
}

/// Create a qualified name from a qualified identifier
impl From<&fpp_ast::QualIdent> for QualifiedName {
    fn from(value: &fpp_ast::QualIdent) -> Self {
        QualifiedName::from_qual_ident(value)
    }
}

/// A qualified name shows as its text, as in [`Display`]
impl Debug for QualifiedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

/// Convert a qualified name to a string
impl Display for QualifiedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let v: Vec<String> = self.to_ident_list().into();
        f.write_str(&v.join("."))
    }
}
