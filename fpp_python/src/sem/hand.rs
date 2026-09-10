//! The single irreducible hand-written escape hatch for the generated semantic
//! wrappers: the `Type` union's `build_type` (a per-hierarchy quirk the uniform
//! generated `dispatch`/`build_*` cannot express) and `Type`'s `__repr__`.
//!
//! The `Type` union is declared `custom_build` in the generated `defs.rs`, so the
//! macro does NOT emit `build_type`; the generated `type_ref` calls
//! `crate::sem::build_type` instead. Everything else — the base/subclasses,
//! `dispatch`, `symbol_ref`, `__eq__`/`__hash__` (`identity identical`), and every
//! field/method getter — is generated. `Type` has no generated `repr` (its
//! `union_directives` leave repr empty), so the `__repr__` here composes with the
//! generated `#[pymethods]` block via PyO3's `multiple-pymethods`.

use crate::model::Model;
use crate::sem::Type;
use fpp_analysis::semantics::Type as SemType;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pymethods;
use std::sync::Arc;

/// Build (dispatching to the concrete subclass) the wrapper for a resolved type.
///
/// The synthetic "unknown" type — a named type whose definition node was never
/// recorded during the walk — is exposed as the bare base `Type` (repr kind
/// "Unknown"); it has no concrete subclass.
pub fn build_type(model: &Py<Model>, py: Python<'_>, ty: Arc<SemType>) -> PyResult<Py<Type>> {
    let data = model.borrow(py).data.clone();
    let unknown = match ty.def_node_id() {
        Some(n) => !data.ids.contains_key(&n),
        None => false,
    };
    let base = Type {
        data,
        model: model.clone_ref(py),
        ty: ty.clone(),
    };
    if unknown {
        return Py::new(py, base);
    }
    Type::dispatch(base, py, &ty)
}

impl Type {
    /// Whether this is the synthetic "unknown" type: a named type whose def node
    /// was never recorded during the walk.
    fn is_unknown(&self) -> bool {
        match self.ty.def_node_id() {
            Some(n) => !self.data.ids.contains_key(&n),
            None => false,
        }
    }
    /// The type-shape name, for `__repr__` only (the public discriminant is the
    /// concrete subclass identity / union alias, not a string).
    fn kind_name(&self) -> &'static str {
        if self.is_unknown() {
            return "Unknown";
        }
        match &*self.ty {
            SemType::PrimitiveInt(_) => "PrimitiveInt",
            SemType::Float(_) => "Float",
            SemType::Boolean => "Boolean",
            SemType::Integer => "Integer",
            SemType::String(_) => "String",
            SemType::AbsType(_) => "AbsType",
            SemType::AliasType(_) => "Alias",
            SemType::Array(_) => "Array",
            SemType::AnonArray(_) => "AnonArray",
            SemType::Enum(_) => "Enum",
            SemType::Struct(_) => "Struct",
            SemType::AnonStruct(_) => "AnonStruct",
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl Type {
    fn __repr__(&self) -> String {
        format!("<Type {}>", self.kind_name())
    }
}

/// Register the hand-written semantic pyclasses. The `Type` union (base +
/// subclasses) is registered by `defs::register`, so nothing hand-authored
/// remains to add here.
pub(crate) fn register(_m: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}
