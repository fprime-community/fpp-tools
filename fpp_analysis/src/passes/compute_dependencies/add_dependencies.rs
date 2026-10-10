use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{BasicUseAnalyzer, UseAnalysisPass};
use crate::passes::compute_dependencies::ComputeDependencies;
use crate::semantics::QualifiedName;
use fpp_ast::*;
use fpp_core::{File, FileReader, Spanned};
use rustc_hash::FxHashSet as HashSet;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Collect the names of the templates that a list of translation units expands.
///
/// The definition of a template and the expansion of that template may appear in
/// different translation units, so the set is collected from the whole list
/// before any dependency is added.
pub struct CollectTemplateExpansions;

impl CollectTemplateExpansions {
    /// The names of the templates expanded in a list of translation units
    pub fn names_in<'a>(tul: impl Iterator<Item = &'a TransUnit>) -> HashSet<String> {
        let mut names = HashSet::default();
        let _ = CollectTemplateExpansions.visit_trans_units(&mut names, tul);
        names
    }
}

impl<'ast> Visitor<'ast> for CollectTemplateExpansions {
    type Break = ();
    type State = HashSet<String>;

    fn super_visit(&self, a: &mut Self::State, node: Node<'ast>) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_spec_template_expand(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<Self::Break> {
        let name: QualifiedName = (&node.template).into();
        a.insert(name.base().to_string());
        ControlFlow::Continue(())
    }
}

/// Add dependencies
pub struct AddDependencies<'ast, Reader: FileReader + Clone> {
    super_: BasicUseAnalyzer<'ast, Analysis, Self>,
    reader: Reader,
    /// The names of the templates that the translation units at this dependency
    /// level expand
    expanded_template_names: HashSet<String>,
}

impl<'ast, Reader: FileReader + Clone> AddDependencies<'ast, Reader> {
    pub fn new(
        reader: Reader,
        expanded_template_names: HashSet<String>,
    ) -> AddDependencies<'ast, Reader> {
        AddDependencies {
            super_: BasicUseAnalyzer::new(),
            reader,
            expanded_template_names,
        }
    }

    /// Should we visit the body of a module template definition?
    ///
    /// The body of a template contributes dependencies only where the template is
    /// expanded. At a deeper dependency level we cannot see the expansions, so
    /// every body is visited.
    fn visit_template_body(&self, a: &Analysis, name: &str) -> bool {
        a.level > 1 || self.expanded_template_names.contains(name)
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

    fn visit_def_module_template(
        &self,
        a: &mut Analysis,
        node: &'ast DefModuleTemplate,
    ) -> ControlFlow<Self::Break> {
        // The parameter list is a source of dependencies wherever the template is
        // defined; the body is one only where the template is expanded.
        self.super_.visit(self, a, Node::DefModuleTemplate(node))?;
        if self.visit_template_body(a, &node.name.data) {
            node.members.visit(a, self)?;
        }
        ControlFlow::Continue(())
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

    fn template_use(
        &self,
        a: &mut Analysis,
        _node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Template, name)
    }

    fn implied_type_use(
        &self,
        a: &mut Analysis,
        _node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Type, name)
    }

    fn implied_constant_use(
        &self,
        a: &mut Analysis,
        _node: &Expr,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Constant, name)
    }

    /// A template argument names a definition wherever the expansion appears,
    /// whether or not the template can be resolved here.
    fn template_arg_node(
        &self,
        a: &mut Analysis,
        node: &'ast TemplateArg,
    ) -> ControlFlow<Self::Break> {
        match &node.kind {
            TemplateArgKind::Constant(e) => e.visit(a, self),
            TemplateArgKind::Type(tn) => tn.visit(a, self),
            TemplateArgKind::Interface(q) => self.interface_instance_use(a, q, q.into()),
        }
    }
}
