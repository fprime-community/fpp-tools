use fpp_core::{SourceFile, Span};

/// The state of a parse
pub(crate) struct ParserState {
    /// The file being parsed
    pub(crate) file: SourceFile,

    /// The including location
    pub(crate) including_loc: Option<Span>,
}
