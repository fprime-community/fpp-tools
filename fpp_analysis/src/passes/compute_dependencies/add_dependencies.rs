use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{BasicUseAnalyzer, UseAnalysisPass};
use crate::passes::compute_dependencies::ComputeDependencies;
use crate::semantics::QualifiedName;
use fpp_ast::*;
use fpp_core::{File, FileReader, Spanned};
use std::ops::ControlFlow;
use std::sync::Arc;

/// Add dependencies
pub struct AddDependencies<'ast, Reader: FileReader + Clone> {
    super_: BasicUseAnalyzer<'ast, Analysis, Self>,
    reader: Reader,
}

impl<'ast, Reader: FileReader + Clone> AddDependencies<'ast, Reader> {
    pub fn new(reader: Reader) -> AddDependencies<'ast, Reader> {
        AddDependencies {
            super_: BasicUseAnalyzer::new(),
            reader,
        }
    }

    /// Descend into a container, tracking the enclosing scope names
    fn scoped(&self, a: &mut Analysis, name: &str, node: Node<'ast>) -> ControlFlow<()> {
        a.scope_name_list.insert(0, name.to_string());
        let result = self.super_.visit(self, a, node);
        a.scope_name_list.remove(0);
        result
    }

    fn analyze_use(
        &self,
        a: &mut Analysis,
        kind: SpecLocKind,
        use_name: QualifiedName,
    ) -> ControlFlow<()> {
        // The candidate names, from the innermost enclosing scope outward
        let mut name_list: Vec<QualifiedName> = Vec::new();
        let use_idents: Vec<String> = use_name.to_ident_list().into();
        for i in (0..=a.scope_name_list.len()).rev() {
            let mut idents: Vec<String> = a.scope_name_list[..i].iter().rev().cloned().collect();
            idents.extend(use_idents.iter().cloned());
            name_list.push(idents.into());
        }
        let spec_loc = name_list
            .into_iter()
            .find_map(|name| a.location_specifier_map.get(&(kind.clone(), name)).cloned());
        match spec_loc {
            Some(spec_loc) => self.add_dependencies(a, &spec_loc),
            None => ControlFlow::Continue(()),
        }
    }

    fn add_dependencies(&self, a: &mut Analysis, spec_loc: &SpecLoc) -> ControlFlow<()> {
        let path = resolve_relative_path(spec_loc);
        let file = File::Path(path);
        if !a.input_file_set.contains(&file) && !a.dependency_file_set.contains(&file) {
            self.add_dependencies_helper(a, file)
        } else {
            ControlFlow::Continue(())
        }
    }

    fn add_dependencies_helper(&self, a: &mut Analysis, file: File) -> ControlFlow<()> {
        a.dependency_file_set.insert(file.clone());
        if a.level == 1 {
            a.direct_dependency_file_set.insert(file.clone());
        }
        let File::Path(path) = &file else {
            return ControlFlow::Continue(());
        };
        let uri = path.to_string_lossy().into_owned();
        let content = match self.reader.read(&uri) {
            Ok(content) => content,
            Err(_) => {
                a.missing_dependency_file_set.insert(file);
                return ControlFlow::Continue(());
            }
        };
        let source_file = fpp_core::SourceFile::new(&uri, content);
        let mut tu = fpp_parser::parse(source_file, |p| p.trans_unit(), None);
        let saved_scope_name_list = std::mem::take(&mut a.scope_name_list);
        let result = ComputeDependencies::new(self.reader.clone()).tu_list(a, &mut [&mut tu]);
        a.scope_name_list = saved_scope_name_list;
        result
    }
}

/// Resolves the path named by a location specifier, relative to the directory
/// of the file containing the specifier
fn resolve_relative_path(spec_loc: &SpecLoc) -> std::path::PathBuf {
    let uri = spec_loc.file.span().file().uri();
    let specified = &spec_loc.file.data;
    match std::path::Path::new(&uri).parent() {
        Some(dir) => File::get_path(&dir.join(specified).to_string_lossy()),
        None => File::get_path(specified),
    }
}

impl<'ast, Reader: FileReader + Clone> Visitor<'ast> for AddDependencies<'ast, Reader> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_def_module(
        &self,
        a: &mut Analysis,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        self.scoped(a, &node.name.data, Node::DefModule(node))
    }

    fn visit_def_component(
        &self,
        a: &mut Analysis,
        node: &'ast DefComponent,
    ) -> ControlFlow<Self::Break> {
        self.scoped(a, &node.name.data, Node::DefComponent(node))
    }

    fn visit_def_state_machine(
        &self,
        a: &mut Analysis,
        node: &'ast DefStateMachine,
    ) -> ControlFlow<Self::Break> {
        self.scoped(a, &node.name.data, Node::DefStateMachine(node))
    }

    fn visit_spec_loc(&self, a: &mut Analysis, node: &'ast SpecLoc) -> ControlFlow<Self::Break> {
        if a.include_dictionary_deps && node.is_dictionary_def {
            // We are visiting a dictionary specifier after visiting
            // the first topology. Add the dependencies for the specifier.
            self.add_dependencies(a, node)
        } else {
            // This is not a dictionary specifier, or we haven't seen a topology.
            // Nothing to do.
            ControlFlow::Continue(())
        }
    }

    fn visit_def_topology(
        &self,
        a: &mut Analysis,
        node: &'ast DefTopology,
    ) -> ControlFlow<Self::Break> {
        // Add dependencies based on explicit and implicit uses in the topology
        self.scoped(a, &node.name.data, Node::DefTopology(node))?;
        // Add dependencies based on dictionary specifiers
        if node.is_deployment && !a.include_dictionary_deps {
            // This is the first deployment topology we have visited.
            // Set include_dictionary_deps and add all dictionary dependencies
            // discovered so far.
            a.include_dictionary_deps = true;
            let dictionary_spec_locs: Vec<Arc<SpecLoc>> = a
                .location_specifier_map
                .values()
                .filter(|s| s.is_dictionary_def)
                .cloned()
                .collect();
            for spec_loc in dictionary_spec_locs {
                self.add_dependencies(a, &spec_loc)?;
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'ast, Reader: FileReader + Clone> UseAnalysisPass<'ast, Analysis>
    for AddDependencies<'ast, Reader>
{
    fn state_machine_use(
        &self,
        a: &mut Analysis,
        _node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::StateMachine, name)
    }

    fn component_use(
        &self,
        a: &mut Analysis,
        _node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Component, name)
    }

    fn constant_use(
        &self,
        a: &mut Analysis,
        _node: &'ast Expr,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        // Analyze as a constant
        self.analyze_use(a, SpecLocKind::Constant, name.clone())?;
        // If in the form A.B, also analyze as an enumerated constant
        let qualifier: Vec<String> = name.qualifier().iter().cloned().collect();
        if qualifier.is_empty() {
            ControlFlow::Continue(())
        } else {
            self.analyze_use(a, SpecLocKind::Type, qualifier.into())
        }
    }

    fn port_use(
        &self,
        a: &mut Analysis,
        _node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Port, name)
    }

    fn interface_instance_use(
        &self,
        a: &mut Analysis,
        _node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Instance, name)
    }

    fn interface_use(
        &self,
        a: &mut Analysis,
        _node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Interface, name)
    }

    fn type_use(
        &self,
        a: &mut Analysis,
        _node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Type, name)
    }
}
