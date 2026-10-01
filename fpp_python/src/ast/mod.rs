//! Compile-time-expanded AST bindings, published as the `ast` submodule.
mod defs;
pub use defs::*;

use pyo3::prelude::*;
use pyo3::types::PyModule;

pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new(py, PY_MODULE)?;
    defs::register(&m)?;
    parent.add("ast", &m)?;
    Ok(())
}
