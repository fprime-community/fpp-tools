use crate::analysis::Analysis;
use crate::passes::{
    BuildSpecLocMap, CheckComponentDefs, CheckComponentInstanceDefs, CheckDictionaryDefs,
    CheckExprTypes, CheckFrameworkConstantValues, CheckFrameworkDefs, CheckInterfaceDefs,
    CheckPortDefs, CheckSpecLocs, CheckStateMachineDefs, CheckSystemDefs,
    CheckTemplateInterfaceArgs, CheckTopologyDefs, CheckTopologyInstances, CheckTypeUses,
    CheckUseDefCycles, CheckUses, ConstructDictionaryMap, ConstructImpliedUseMap,
    EvalConstantExprs, EvalImpliedEnumConsts, ExpandSyntaxAndEnterSymbols, FinalizeConstantExprs,
    FinalizeTypeDefs,
};
use fpp_ast::{TransUnit, Visitor};
use fpp_core::FileReader;
use std::ops::ControlFlow;

/// Check semantics for a list of translation units
pub struct CheckSemantics;

impl CheckSemantics {
    pub fn tu_list<R: FileReader>(
        a: &mut Analysis,
        reader: R,
        tul: &mut [TransUnit],
    ) -> ControlFlow<()> {
        ExpandSyntaxAndEnterSymbols::tu_list(a, reader, tul)?;
        Self::check_expanded_tu_list(a, &tul.iter().collect::<Vec<_>>())
    }

    /// The steps of [`CheckSemantics::tu_list`] that follow
    /// [`ExpandSyntaxAndEnterSymbols::tu_list`]: check the semantics of the
    /// expanded translation units, whose symbols are entered in `a`.
    pub fn check_expanded_tu_list(a: &mut Analysis, tul: &[&TransUnit]) -> ControlFlow<()> {
        let tul = || tul.iter().copied();
        ConstructImpliedUseMap.visit_trans_units(a, tul())?;
        CheckUses::new().visit_trans_units(a, tul())?;
        // The analysis CheckUseDefCycles computes is discarded: the later passes
        // see the use-def bookkeeping as it was before the cycle check
        let visited_symbol_set = std::mem::take(&mut a.visited_symbol_set);
        let use_def_symbol_set = std::mem::take(&mut a.use_def_symbol_set);
        let use_def_matching_list = std::mem::take(&mut a.use_def_matching_list);
        let result = CheckUseDefCycles::new().visit_trans_units(a, tul());
        a.visited_symbol_set = visited_symbol_set;
        a.use_def_symbol_set = use_def_symbol_set;
        a.use_def_matching_list = use_def_matching_list;
        result?;
        CheckTypeUses::new().visit_trans_units(a, tul())?;
        CheckExprTypes::new().visit_trans_units(a, tul())?;
        CheckFrameworkDefs.visit_trans_units(a, tul())?;
        EvalImpliedEnumConsts::new().visit_trans_units(a, tul())?;
        EvalConstantExprs::new().visit_trans_units(a, tul())?;
        FinalizeTypeDefs::new().visit_trans_units(a, tul())?;
        FinalizeConstantExprs::new().visit_trans_units(a, tul())?;
        CheckFrameworkConstantValues.check(a);
        CheckPortDefs.visit_trans_units(a, tul())?;
        CheckInterfaceDefs.visit_trans_units(a, tul())?;
        CheckComponentDefs.visit_trans_units(a, tul())?;
        CheckComponentInstanceDefs.visit_trans_units(a, tul())?;
        CheckComponentInstanceDefs::check_id_ranges(a);
        CheckStateMachineDefs.visit_trans_units(a, tul())?;
        CheckTopologyInstances.visit_trans_units(a, tul())?;
        CheckTopologyDefs.visit_trans_units(a, tul())?;
        CheckTemplateInterfaceArgs.check(a);
        BuildSpecLocMap.visit_trans_units(a, tul())?;
        CheckSpecLocs.visit_trans_units(a, tul())?;
        CheckDictionaryDefs.visit_trans_units(a, tul())?;
        ConstructDictionaryMap.visit_trans_units(a, tul())?;
        CheckSystemDefs.visit_trans_units(a, tul())?;

        ControlFlow::Continue(())
    }
}
