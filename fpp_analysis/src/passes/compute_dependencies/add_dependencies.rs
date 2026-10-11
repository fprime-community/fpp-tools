use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{BasicUseAnalyzer, UseAnalysisPass, visit_template_arg};
use crate::passes::compute_dependencies::ComputeDependencies;
use crate::semantics::{QualifiedName, Symbol};
use fpp_ast::*;
use fpp_core::{Error, File, FileReader, SourceFile, Spanned};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::cell::Cell;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Add dependencies
pub struct AddDependencies;

impl AddDependencies {
    /// Add the dependencies of a list of translation units.
    /// The set of expanded template names must be collected from the whole list,
    /// because the definition of a template and the expansion of that template
    /// may appear in different translation units.
    pub fn tu_list<Reader: FileReader>(
        reader: &Reader,
        a: &mut Analysis,
        tul: &[TransUnit],
    ) -> ControlFlow<()> {
        let analyzer = TuAnalyzer::new(reader, CollectTemplateExpansions::names_in(tul.iter()));
        analyzer.visit_trans_units(a, tul.iter())
    }
}

/// Collect the names of the templates expanded in a list of translation units
struct CollectTemplateExpansions;

impl CollectTemplateExpansions {
    /// Get the names of the templates expanded in a list of translation units
    fn names_in<'a>(tul: impl Iterator<Item = &'a TransUnit>) -> HashSet<String> {
        let mut names = HashSet::default();
        let _ = CollectTemplateExpansions.visit_trans_units(&mut names, tul);
        names
    }
}

impl<'ast> Visitor<'ast> for CollectTemplateExpansions {
    type Break = ();
    type State = HashSet<String>;

    fn super_visit(&self, s: &mut Self::State, node: Node<'ast>) -> ControlFlow<Self::Break> {
        node.walk(s, self)
    }

    fn visit_spec_template_expand(
        &self,
        s: &mut Self::State,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<Self::Break> {
        let name: QualifiedName = (&node.template).into();
        s.insert(name.base().to_string());
        node.members.visit(s, self)
    }
}

/// Add the dependencies of the translation units at one dependency level.
/// `expanded_template_names` is the set of names of the module templates that
/// those translation units expand.
struct TuAnalyzer<'ast, 'r, Reader: FileReader> {
    super_: BasicUseAnalyzer<'ast, Self>,
    reader: &'r Reader,
    expanded_template_names: HashSet<String>,
}

impl<'ast, Reader: FileReader> Visitor<'ast> for TuAnalyzer<'ast, '_, Reader> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
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
        self.super_.visit(self, a, Node::DefTopology(node))?;
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
        // Otherwise this is the second or later deployment topology, or a
        // non-deployment topology; nothing to do
        ControlFlow::Continue(())
    }

    fn visit_def_module_template(
        &self,
        a: &mut Analysis,
        node: &'ast DefModuleTemplate,
    ) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, Node::DefModuleTemplate(node))?;
        if self.visit_template_body(a, &node.name.data) {
            node.members.visit(a, self)?;
        }
        ControlFlow::Continue(())
    }

    fn visit_spec_template_expand(
        &self,
        a: &mut Analysis,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, Node::SpecTemplateExpand(node))?;
        for arg in &node.args {
            match &arg.kind {
                TemplateArgKind::Constant(value) => value.visit(a, self)?,
                TemplateArgKind::Type(type_name) => type_name.visit(a, self)?,
                TemplateArgKind::Interface(instance) => {
                    self.interface_instance_use(a, instance, instance.into())?
                }
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'ast, Reader: FileReader> UseAnalysisPass<'ast, Analysis> for TuAnalyzer<'ast, '_, Reader> {
    fn template_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Template, use_, node.is_absolute())
    }

    fn state_machine_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::StateMachine, use_, node.is_absolute())
    }

    fn component_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Component, use_, node.is_absolute())
    }

    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &Expr,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let is_absolute = node
            .get_is_absolute_opt()
            .expect("a constant use is a qualified identifier");
        self.analyze_constant_use(a, use_, is_absolute)
    }

    fn port_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Port, use_, node.is_absolute())
    }

    fn interface_instance_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Instance, use_, node.is_absolute())
    }

    fn interface_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.analyze_use(a, SpecLocKind::Interface, use_, node.is_absolute())
    }

    fn type_use(
        &self,
        a: &mut Analysis,
        node: &TypeName,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let is_absolute = node
            .get_is_absolute_opt()
            .expect("a type use is a qualified identifier");
        self.analyze_use(a, SpecLocKind::Type, use_, is_absolute)
    }

    fn template_constant_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }

    fn template_type_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }

    fn template_interface_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }
}

impl<'ast, 'r, Reader: FileReader> TuAnalyzer<'ast, 'r, Reader> {
    fn new(
        reader: &'r Reader,
        expanded_template_names: HashSet<String>,
    ) -> TuAnalyzer<'ast, 'r, Reader> {
        TuAnalyzer {
            super_: BasicUseAnalyzer::new(),
            reader,
            expanded_template_names,
        }
    }

    /// Should we visit the body of a module template definition?
    fn visit_template_body(&self, a: &Analysis, name: &str) -> bool {
        a.level > 1 || self.expanded_template_names.contains(name)
    }

    fn analyze_use(
        &self,
        a: &mut Analysis,
        kind: SpecLocKind,
        use_: QualifiedName,
        is_absolute: bool,
    ) -> ControlFlow<()> {
        let name_list = Self::compute_name_list(a, &use_, is_absolute);
        let location = Self::find_location(a, kind, name_list);
        match location {
            Some(spec_loc) => self.add_dependencies(a, &spec_loc),
            None => ControlFlow::Continue(()),
        }
    }

    /// The names a use may refer to: the use qualified by each suffix of the
    /// scope name list, outermost names first, then the use as written. An
    /// absolute use refers only to the use as written.
    fn compute_name_list(
        a: &Analysis,
        use_: &QualifiedName,
        is_absolute: bool,
    ) -> Vec<QualifiedName> {
        if is_absolute {
            return vec![use_.clone()];
        }
        let use_idents = use_.to_ident_list();
        let mut result: Vec<QualifiedName> = (0..a.scope_name_list.len())
            .map(|k| {
                let mut idents: Vec<String> =
                    a.scope_name_list[k..].iter().rev().cloned().collect();
                idents.extend(use_idents.iter().cloned());
                QualifiedName::from(idents)
            })
            .collect();
        result.push(use_.clone());
        result
    }

    fn find_location(
        a: &Analysis,
        kind: SpecLocKind,
        name_list: Vec<QualifiedName>,
    ) -> Option<Arc<SpecLoc>> {
        name_list
            .into_iter()
            .find_map(|name| a.location_specifier_map.get(&(kind.clone(), name)).cloned())
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
        // A dependency file that cannot be opened, or that includes a file that
        // cannot be opened, is a missing dependency
        let content = match self.reader.read(&uri) {
            Ok(content) => content,
            Err(_) => {
                a.missing_dependency_file_set.insert(file);
                return ControlFlow::Continue(());
            }
        };
        let mut tu = fpp_parser::parse(SourceFile::new(&uri, content), |p| p.trans_unit(), None);
        let cannot_open = Cell::new(false);
        let mut include_context_map = HashMap::default();
        let _ = fpp_parser::ResolveIncludes::new(CannotOpenReader {
            reader: self.reader,
            cannot_open: &cannot_open,
        })
        .visit_trans_unit(&mut include_context_map, &mut tu);
        if cannot_open.get() {
            a.missing_dependency_file_set.insert(file);
            return ControlFlow::Continue(());
        }
        a.include_context_map.extend(include_context_map);
        let scope_name_list = std::mem::take(&mut a.scope_name_list);
        let result = ComputeDependencies::new(self.reader).tu_list(a, std::slice::from_ref(&tu));
        a.scope_name_list = scope_name_list;
        result
    }

    fn add_dependencies(&self, a: &mut Analysis, spec_loc: &SpecLoc) -> ControlFlow<()> {
        let loc = spec_loc.file.span();
        let path = loc.get_relative_path(&spec_loc.file.data);
        let file = File::Path(path);
        if !a.input_file_set.contains(&file) && !a.dependency_file_set.contains(&file) {
            self.add_dependencies_helper(a, file)
        } else {
            ControlFlow::Continue(())
        }
    }

    /// A use of a constant, which may also be a use of an enumerated constant
    fn analyze_constant_use(
        &self,
        a: &mut Analysis,
        use_: QualifiedName,
        is_absolute: bool,
    ) -> ControlFlow<()> {
        // Analyze as a constant
        self.analyze_use(a, SpecLocKind::Constant, use_.clone(), is_absolute)?;
        // If in the form A.B, also analyze as an enumerated constant
        let qualifier: Vec<String> = use_.qualifier().iter().cloned().collect();
        if qualifier.is_empty() {
            ControlFlow::Continue(())
        } else {
            self.analyze_use(a, SpecLocKind::Type, qualifier.into(), is_absolute)
        }
    }
}

/// A reader for the files that a dependency file includes. A file that cannot
/// be opened reads as empty, and the reader records that it could not be
/// opened.
struct CannotOpenReader<'r, 'c, Reader: FileReader> {
    reader: &'r Reader,
    cannot_open: &'c Cell<bool>,
}

impl<Reader: FileReader> FileReader for CannotOpenReader<'_, '_, Reader> {
    fn resolve(&self, current: SourceFile, include: &str) -> Result<String, Error> {
        match self.reader.resolve(current, include) {
            Ok(path) => Ok(path),
            Err(_) => {
                self.cannot_open.set(true);
                Ok(String::new())
            }
        }
    }

    fn read(&self, path: &str) -> Result<String, Error> {
        match self.reader.read(path) {
            Ok(content) => Ok(content),
            Err(_) => {
                self.cannot_open.set(true);
                Ok(String::new())
            }
        }
    }
}
