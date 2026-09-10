use fpp_core::Span;

/// A recorded location specifier, keyed in `location_specifier_map`.
#[derive(Debug, Clone)]
pub struct SpecLocEntry {
    /// Span of the location specifier statement
    pub spec_span: Span,
    /// Span of the file string literal (error location + base for path resolution)
    pub file_span: Span,
    /// The specified (relative) path string
    pub file_value: String,
    /// Whether this is a dictionary specifier
    pub is_dictionary_def: bool,
}
