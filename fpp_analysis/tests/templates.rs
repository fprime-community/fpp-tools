//! Tests for template expansion.
//!
//! The `integration` suite compares diagnostics; these tests instead assert what
//! an expansion puts into the model: that each expansion is a distinct copy of
//! the template body, that a use inside it resolves to the bound argument, and
//! that a name in the body resolves at the point of the template definition.

use fpp_analysis::semantics::{Connection, Symbol, Topology, Type};
use fpp_analysis::{Analysis, CheckSemantics, ExpandSyntaxAndEnterSymbols};
use fpp_ast::{
    DefAliasType, DefConstant, ModuleMember, MoveWalkable, Node, TransUnit, Visitable, Visitor,
};
use fpp_core::{FileReader, SourceFile};
use fpp_fs::FsReader;
use rustc_hash::FxHashSet as HashSet;
use std::ops::ControlFlow;
use std::path::PathBuf;

/// Parse, expand templates and check semantics, then hand the analysis and the
/// expanded translation unit to `f`. Panics if any step produced a diagnostic.
fn with_analysis<R>(src: &str, f: impl FnOnce(&Analysis, &TransUnit) -> R) -> R {
    let mut diagnostics = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
    let out = fpp_core::run(&mut ctx, || {
        let source = SourceFile::new("templates_test.fpp", src.to_string());
        let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
        let mut a = Analysis::new();
        let _ = CheckSemantics::tu_list(&mut a, FsReader {}, std::slice::from_mut(&mut ast));
        f(&a, &ast)
    });
    let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
    assert_eq!(output, "", "expected no diagnostics");
    out
}

/// Collects the identity of every node it visits
struct CollectNodes;

/// The node identities seen, and the identities seen more than once
#[derive(Default)]
struct SeenNodes {
    seen: HashSet<fpp_core::Node>,
    duplicates: Vec<fpp_core::Node>,
}

impl<'ast> Visitor<'ast> for CollectNodes {
    type Break = ();
    type State = SeenNodes;

    fn super_visit(&self, a: &mut SeenNodes, node: Node<'ast>) -> ControlFlow<Self::Break> {
        // A node kind that is a union of node kinds takes its identity from the
        // node it wraps, so counting both would report every one of them twice.
        let is_union = matches!(
            node,
            Node::QualIdent(_) | Node::SpecPortInstance(_) | Node::TemplateParam(_)
        );
        if !is_union {
            let id = fpp_ast::AstNode::id(&node);
            if !a.seen.insert(id) {
                a.duplicates.push(id);
            }
        }
        node.walk(a, self)
    }
}

/// The flattened members of a translation unit or module: an expansion
/// contributes the members it introduced
fn flatten(members: &[ModuleMember]) -> Vec<&ModuleMember> {
    let mut out = Vec::new();
    for member in members {
        match member {
            ModuleMember::SpecTemplateExpand(spec) => {
                if let Some(inner) = &spec.members {
                    out.extend(flatten(inner))
                }
            }
            _ => out.push(member),
        }
    }
    out
}

/// The flattened members of the module at the given path. Two modules with the
/// same name are the same module, so the members of all of them are returned.
fn members_at<'a>(ast: &'a TransUnit, path: &[&str]) -> Vec<&'a ModuleMember> {
    let mut members = flatten(&ast.0);
    for name in path {
        let mut next: Vec<&ModuleMember> = Vec::new();
        for member in members {
            if let ModuleMember::DefModule(def) = member
                && def.name.data == *name
            {
                next.extend(flatten(&def.members));
            }
        }
        members = next;
    }
    members
}

/// The unique constant definition at the given dotted path
fn constant_at<'a>(ast: &'a TransUnit, path: &str) -> &'a DefConstant {
    let parts: Vec<&str> = path.split('.').collect();
    let (name, prefix) = parts.split_last().expect("a nonempty path");
    let found: Vec<&DefConstant> = members_at(ast, prefix)
        .into_iter()
        .filter_map(|m| match m {
            ModuleMember::DefConstant(def) if def.name.data == *name => Some(def),
            _ => None,
        })
        .collect();
    match found.as_slice() {
        [def] => def,
        [] => panic!("there is no definition {path}"),
        _ => panic!("there is more than one definition {path}"),
    }
}

/// The unique alias type definition at the given dotted path
fn alias_type_at<'a>(ast: &'a TransUnit, path: &str) -> &'a DefAliasType {
    let parts: Vec<&str> = path.split('.').collect();
    let (name, prefix) = parts.split_last().expect("a nonempty path");
    let found: Vec<&DefAliasType> = members_at(ast, prefix)
        .into_iter()
        .filter_map(|m| match m {
            ModuleMember::DefAliasType(def) if def.name.data == *name => Some(def),
            _ => None,
        })
        .collect();
    match found.as_slice() {
        [def] => def,
        [] => panic!("there is no definition {path}"),
        _ => panic!("there is more than one definition {path}"),
    }
}

/// The resolved topology with the given qualified name
fn topology_at<'a>(a: &'a Analysis, name: &str) -> &'a Topology {
    a.topology_map
        .values()
        .find(|top| top.qualified_name == name)
        .unwrap_or_else(|| panic!("there is no resolved topology {name}"))
}

/// The integer value of a node
fn int_value(a: &Analysis, node: fpp_core::Node) -> i128 {
    a.get_int_value(node)
        .unwrap_or_else(|| panic!("node should have an integer value"))
}

/// The qualified names of the definitions known to the analysis. A bound
/// template parameter is not a global definition, so it is excluded.
fn qualified_names(a: &Analysis) -> HashSet<String> {
    let mut symbols: HashSet<Symbol> = HashSet::default();
    symbols.extend(a.parent_symbol_map.keys().cloned());
    symbols.extend(a.parent_symbol_map.values().cloned());
    symbols.extend(a.symbol_scope_map.keys().cloned());
    symbols.extend(a.use_def_map.values().cloned());
    symbols
        .iter()
        .filter(|s| !s.is_template_arg())
        .map(|s| a.get_qualified_name(s))
        .collect()
}

const MODEL: &str = "\
module template T(constant c: U32, type Ty) {
    constant k = c
    type Alias = Ty
    module Inner {
        constant j = c + 1
    }
}

module M1 {
    expand T(constant 1, type U32)
}

module M2 {
    expand T(constant 2, type I16)
}
";

#[test]
fn expansion_introduces_the_definitions_of_the_body_at_the_expansion_site() {
    with_analysis(MODEL, |a, ast| {
        let names = qualified_names(a);
        for name in [
            "M1.k",
            "M1.Alias",
            "M1.Inner",
            "M1.Inner.j",
            "M2.k",
            "M2.Alias",
            "M2.Inner",
            "M2.Inner.j",
        ] {
            assert!(names.contains(name), "missing definition {name}");
        }
        // The definition of the template introduces nothing by itself
        assert!(
            !names.contains("k"),
            "the template body defined k at top level"
        );
        assert!(!names.contains("T.k"), "the template body defined T.k");
        // Each expansion introduces exactly the three members of the body
        assert_eq!(members_at(ast, &["M1"]).len(), 3);
        assert_eq!(members_at(ast, &["M2"]).len(), 3);
    });
}

#[test]
fn each_expansion_gets_its_own_copy_of_the_definitions() {
    with_analysis(MODEL, |a, ast| {
        assert_eq!(int_value(a, constant_at(ast, "M1.k").node_id), 1);
        assert_eq!(int_value(a, constant_at(ast, "M2.k").node_id), 2);
        assert_eq!(int_value(a, constant_at(ast, "M1.Inner.j").node_id), 2);
        assert_eq!(int_value(a, constant_at(ast, "M2.Inner.j").node_id), 3);
    });
}

#[test]
fn a_use_of_a_constant_parameter_resolves_to_the_bound_argument() {
    with_analysis(MODEL, |a, ast| {
        let def = constant_at(ast, "M1.k");
        match a.use_def_map.get(&def.value.node_id) {
            Some(Symbol::TemplateConstantArg { param_def, value }) => {
                assert_eq!(param_def.name.data, "c");
                assert_eq!(int_value(a, value.node_id), 1);
            }
            Some(symbol) => panic!(
                "c resolved to {}, not to the bound parameter",
                a.get_qualified_name(symbol)
            ),
            None => panic!("the use of c is not resolved"),
        }
    });
}

#[test]
fn a_use_of_a_type_parameter_resolves_to_the_bound_argument() {
    with_analysis(MODEL, |a, ast| {
        let def = alias_type_at(ast, "M2.Alias");
        let qual_ident = match &def.type_name.kind {
            fpp_ast::TypeNameKind::QualIdent(q) => q,
            other => panic!("expected a qualified identifier type name, got {other:?}"),
        };
        use fpp_ast::AstNode;
        match a.use_def_map.get(&qual_ident.id()) {
            Some(Symbol::TemplateTypeArg { param_def, value }) => {
                assert_eq!(param_def.name.data, "Ty");
                assert!(matches!(
                    a.type_map.get(&value.node_id).map(|t| t.as_ref()),
                    Some(Type::PrimitiveInt(fpp_ast::IntegerKind::I16))
                ));
            }
            Some(symbol) => panic!(
                "Ty resolved to {}, not to the bound parameter",
                a.get_qualified_name(symbol)
            ),
            None => panic!("the use of Ty is not resolved"),
        }
    });
}

#[test]
fn a_name_in_the_body_resolves_at_the_point_of_the_definition() {
    let src = "\
module A {
    constant x = 1
    module template T() {
        constant y = x
    }
}

module B {
    constant x = 2
    expand A.T()
}
";
    with_analysis(src, |a, ast| {
        let def = constant_at(ast, "B.y");
        match a.use_def_map.get(&def.value.node_id) {
            Some(symbol) => assert_eq!(
                a.get_qualified_name(symbol),
                "A.x",
                "x resolved to {}, not to A.x",
                a.get_qualified_name(symbol)
            ),
            None => panic!("the use of x is not resolved"),
        }
        // A.x is 1 and B.x is 2, so the value of B.y shows which one won
        assert_eq!(int_value(a, def.node_id), 1);
    });
}

/// A module defined by an expansion is merged with a same-named module at the
/// expansion site, so that both sets of members are visible from outside. But a
/// use inside the expansion must see only the members that the expansion
/// introduced: otherwise a definition at the expansion site captures a use of a
/// bound parameter, and the argument is silently discarded.
#[test]
fn a_same_named_module_at_the_expansion_site_does_not_capture_a_parameter() {
    let src = "\
module template T(constant c: U32) {
    module Inner {
        constant k = c
    }
}

module M {
    module Inner {
        constant c = 7
    }

    expand T(constant 1)
}
";
    with_analysis(src, |a, ast| {
        let def = constant_at(ast, "M.Inner.k");
        match a.use_def_map.get(&def.value.node_id) {
            Some(Symbol::TemplateConstantArg { param_def, value }) => {
                assert_eq!(param_def.name.data, "c");
                assert_eq!(int_value(a, value.node_id), 1);
            }
            Some(symbol) => panic!(
                "c resolved to {}, not to the bound parameter",
                a.get_qualified_name(symbol)
            ),
            None => panic!("the use of c is not resolved"),
        }
        // The bound parameter is 1 and M.Inner.c is 7, so the value of M.Inner.k
        // shows which one won
        assert_eq!(int_value(a, def.node_id), 1);
        // Both modules named Inner are the same module, so both sets of members
        // are visible from outside the expansion
        let names = qualified_names(a);
        assert!(names.contains("M.Inner.k"));
        assert!(names.contains("M.Inner.c"));
    });
}

/// Expand the templates of the model `src`, read from `path`, and check that
/// every node in the result has its own identity. Panics if the expansion
/// produced any diagnostic.
///
/// A value of a bound parameter is shared by every use of it, so a copy must not
/// share a node identity with the syntax it was copied from, or with another
/// copy. Returns the number of nodes in the expanded model.
fn check_expand_unique(path: &str, src: String) -> usize {
    let mut diagnostics = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
    let count = fpp_core::run(&mut ctx, || {
        let source = SourceFile::new(path, src);
        let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
        let mut a = Analysis::new();
        let result = ExpandSyntaxAndEnterSymbols::tu_list(
            &mut a,
            FsReader {},
            std::slice::from_mut(&mut ast),
        );
        assert!(result.is_continue(), "the expansion should succeed");

        let mut seen = SeenNodes::default();
        let _ = ast.visit(&mut seen, &CollectNodes);
        assert!(
            seen.duplicates.is_empty(),
            "{} nodes are shared between the template body and its expansions",
            seen.duplicates.len()
        );
        seen.seen.len()
    });
    let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
    assert_eq!(output, "", "expected no diagnostics");
    count
}

/// Three expansions of a template with a string parameter are distinct copies
#[test]
fn simple() {
    let src = r#"
module template T(constant c: string) {
    constant f = c
}

module M1 {
    expand T(constant "a")
}

module M2 {
    expand T(constant "b")
}

module M3 {
    expand T(constant "c")
}
"#;
    check_expand_unique("templates_test.fpp", src.to_string());
}

/// Expand each model in `tests/templates/ok`, and check that every node in the
/// result has its own identity. The model there includes a broad slice of the
/// grammar into a template body and expands it twice.
#[test]
fn ok() {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("tests/templates/ok");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the templates/ok directory")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "fpp"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "templates/ok should hold some models");
    for file in files {
        let path = file.to_str().expect("a UTF-8 path");
        let content = FsReader {}.read(path).expect("the model file");
        let count = check_expand_unique(path, content);
        assert!(count > 100, "{path} should be substantial");
    }
}

/// Two expansions of one template each contribute a connection copied from the
/// same body. The copies are distinct connections, so each gets its own port
/// number on the output port array they share.
#[test]
fn two_expansions_contribute_two_connections_at_one_output_port() {
    let src = "\
port P

passive component C {
    sync input port pIn: [4] P
}

passive component Producer {
    output port pOut: [4] P
}

instance c1: C base id 0x100

instance p1: Producer base id 0x300

interface I {
    sync input port pIn: [4] P
}

module template T(instance i: I) {
    topology Sub {
        instance i
        instance p1
        connections C1 {
            p1.pOut -> i.pIn
        }
    }
}

module E1 {
    expand T(instance c1)
}

module E2 {
    expand T(instance c1)
}

topology Dep {
    import E1.Sub
    import E2.Sub
}
";
    with_analysis(src, |a, _| {
        let top = topology_at(a, "Dep");
        let connections: Vec<&Connection> = top
            .output_connection_map
            .values()
            .flat_map(|cs| cs.iter())
            .collect();
        assert_eq!(
            connections.len(),
            2,
            "the two copies of the connection collapsed into one"
        );
        let port_numbers: Vec<i128> = top.from_port_number_map.values().copied().collect();
        assert_eq!(port_numbers, vec![0, 1]);
    });
}

/// A named array type is the type of an expression only when it is the declared
/// type of a constant parameter, so this is the only way to promote a scalar to
/// an array whose size is not yet known. The promoted value carries its element
/// as a scalar until the size is known, and every conversion it goes through on
/// the way must keep it.
#[test]
fn a_scalar_promoted_to_an_array_of_unknown_size_keeps_its_element() {
    let src = "\
array B = [2] U32

module template T(constant b: B) {
    constant inner = [ 1, b ]
    constant outer = [ inner, inner ]
    constant first = outer[0][0][0]
    constant second = outer[0][0][1]
    constant third = outer[0][1][0]
}

module M {
    expand T(constant [2, 3])
}
";
    with_analysis(src, |a, ast| {
        // outer[0][0] is the scalar 1 promoted to a B, so each of its elements
        // is 1
        assert_eq!(int_value(a, constant_at(ast, "M.first").node_id), 1);
        assert_eq!(int_value(a, constant_at(ast, "M.second").node_id), 1);
        // outer[0][1] is the bound argument
        assert_eq!(int_value(a, constant_at(ast, "M.third").node_id), 2);
    });
}
