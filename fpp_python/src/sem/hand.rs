//! The single irreducible hand-written escape hatch for the generated semantic
//! wrappers: the `Type` union's `build_type`, a per-hierarchy quirk the uniform
//! generated `dispatch`/`build_*` cannot express.
//!
//! The `Type` union is declared `custom_build` in the generated `defs.rs`, so the
//! macro does NOT emit `build_type`; the generated `type_ref` calls
//! `crate::sem::build_type` instead. Everything else — the base/subclasses,
//! `dispatch`, `symbol_ref`, `__eq__`/`__hash__` (`identity identical`),
//! `__repr__`/`__str__` (`repr display`, i.e. the native `Display`), and every
//! field/method getter — is generated.

use crate::model::Model;
use crate::sem::Type;
use fpp_analysis::semantics::Type as SemType;
use pyo3::prelude::*;
use std::sync::Arc;

/// Build (dispatching to the concrete subclass) the wrapper for a resolved type.
///
/// The synthetic "unknown" type — a named type whose definition node was never
/// recorded during the walk.
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

/// Register the hand-written semantic pyclasses. The `Type` union (base +
/// subclasses) is registered by `defs::register`, so nothing hand-authored
/// remains to add here.
pub(crate) fn register(_m: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}
