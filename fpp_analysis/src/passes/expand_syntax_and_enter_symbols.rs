use crate::analysis::Analysis;
use crate::passes::{
    CheckTemplateDefs, CheckTemplateExpansions, CheckTemplateUses, EnterSymbols,
    EnterTemplateExpansionSymbols,
};
use crate::transform::ExpandTemplates;
use fpp_ast::{TransUnit, Visitor};
use fpp_core::{Error, FileReader, SourceFile};
use std::ops::ControlFlow;

/// Perform all syntax expansion required for analysis.
/// Because template expansion requires def-use analysis,
/// also enter all symbols into their scopes
pub struct ExpandSyntaxAndEnterSymbols;

impl ExpandSyntaxAndEnterSymbols {
    pub fn tu_list<R: FileReader>(
        a: &mut Analysis,
        reader: R,
        tul: &mut [TransUnit],
    ) -> ControlFlow<()> {
        for tu in tul.iter_mut() {
            crate::resolve_includes(a, ReaderRef(&reader), tu)?;
        }
        for tu in tul.iter_mut() {
            crate::add_state_enums(tu);
        }
        Self::enter_symbols_and_expand_templates(a, tul)
    }

    /// The steps of [`ExpandSyntaxAndEnterSymbols::tu_list`] that follow include
    /// resolution and the synthesis of the state enums: enter the symbols, check
    /// the template definitions and the expanded templates, match each template
    /// expansion specifier to its template, expand the templates, and enter the
    /// symbols the expansions introduce.
    ///
    /// The language server parses each file once and keeps the parsed unit, with
    /// its includes resolved and its state enums added, across analyses. Each
    /// analysis then starts from these steps.
    pub fn enter_symbols_and_expand_templates(
        a: &mut Analysis,
        tul: &mut [TransUnit],
    ) -> ControlFlow<()> {
        EnterSymbols.visit_trans_units(a, tul.iter())?;
        CheckTemplateDefs.visit_trans_units(a, tul.iter())?;
        CheckTemplateExpansions.visit_trans_units(a, tul.iter())?;
        CheckTemplateUses.visit_trans_units(a, tul.iter())?;
        if let Err(err) = ExpandTemplates::transform_list(a, tul) {
            err.emit();
            return ControlFlow::Break(());
        }
        EnterTemplateExpansionSymbols.visit_trans_units(a, tul.iter())
    }
}

/// A borrowed [`FileReader`], for a [`fpp_parser::ResolveIncludes`], which takes
/// its reader by value
pub(crate) struct ReaderRef<'r, R: FileReader>(pub(crate) &'r R);

impl<R: FileReader> FileReader for ReaderRef<'_, R> {
    fn resolve(&self, current: SourceFile, include: &str) -> Result<String, Error> {
        self.0.resolve(current, include)
    }

    fn read(&self, path: &str) -> Result<String, Error> {
        self.0.read(path)
    }
}
