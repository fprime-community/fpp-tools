use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::Type;
use fpp_ast::SpecRecord;
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP data product record
#[derive(Debug, Clone)]
pub struct Record {
    pub node: Arc<SpecRecord>,
    pub record_type: Arc<Type>,
    pub is_array: bool,
}

/// A record ID
pub type RecordId = i128;

impl Record {
    /// Gets the name of the record
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the record
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates a record from a record specifier
    pub fn from_spec_record(a: &Analysis, node: &SpecRecord) -> SemanticResult<Record> {
        let record_type = a.get_finalized_type(node.record_type.node_id).unwrap();
        a.check_displayable_type(
            node.record_type.node_id,
            node.record_type.span(),
            "type of record is not displayable",
        )?;
        Ok(Record {
            node: Arc::new(node.clone()),
            record_type,
            is_array: node.is_array,
        })
    }
}
