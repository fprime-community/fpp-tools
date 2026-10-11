//! A mutable visitor can traverse the whole tree through its default hook.

use fpp_ast::{ModuleMember, MutVisitor, MutWalkable, Name};
use fpp_core::{CompilerContext, DiagnosticData, DiagnosticEmitter, SourceFile};
use fpp_parser::parse;
use std::ops::ControlFlow;

struct NoEmitter;

impl DiagnosticEmitter for NoEmitter {
    fn emit(&mut self, _diagnostic: DiagnosticData) {}
}

/// Upper-cases every definition name, wherever it is
struct UpperCaseNames;

impl MutVisitor for UpperCaseNames {
    type Break = ();
    type State = ();

    fn super_visit_mut<V: MutWalkable<Self>>(&self, a: &mut (), node: &mut V) -> ControlFlow<()> {
        node.walk_mut(a, self)
    }

    fn visit_name(&self, _: &mut (), node: &mut Name) -> ControlFlow<()> {
        node.data = node.data.to_uppercase();
        ControlFlow::Continue(())
    }
}

#[test]
fn deep_traversal() {
    fpp_core::run(&mut CompilerContext::new(NoEmitter), || {
        let source = "module m {\n  constant c = 0\n  module n {\n    array a = [1] U32\n  }\n}\n";
        let file = SourceFile::new("deep.fpp", source.to_string());
        let mut tu = parse(file, |p| p.trans_unit(), None);
        let _ = UpperCaseNames.visit_trans_unit(&mut (), &mut tu);

        let ModuleMember::DefModule(m) = &tu.0[0] else {
            panic!("expected a module")
        };
        assert_eq!(m.name.data, "M");
        let ModuleMember::DefConstant(c) = &m.members[0] else {
            panic!("expected a constant")
        };
        assert_eq!(c.name.data, "C");
        let ModuleMember::DefModule(n) = &m.members[1] else {
            panic!("expected a module")
        };
        let ModuleMember::DefArray(a) = &n.members[0] else {
            panic!("expected an array")
        };
        assert_eq!((n.name.data.as_str(), a.name.data.as_str()), ("N", "A"));
    });
}
