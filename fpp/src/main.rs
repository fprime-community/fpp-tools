use std::io::Read;
use std::process::exit;

fn compiler_main() -> String {
    let mut stdin = String::new();
    std::io::stdin()
        .read_to_string(&mut stdin)
        .expect("Failed to read input stream");

    let src = fpp_core::SourceFile::new("<stdin>", stdin);
    let mut ast = fpp_parser::parse(src, |p| p.trans_unit(), None);

    let mut a = fpp_analysis::Analysis::new();
    a.input_file_set.insert(fpp_core::File::StdIn);

    let _ = fpp_analysis::resolve_includes(&mut a, fpp_fs::FsReader {}, &mut ast);
    fpp_analysis::add_state_enums(&mut ast);
    let _ = fpp_analysis::check_semantics(&mut a, vec![&ast]);

    format!("{:#?}", ast)
}

fn main() {
    let mut diagnostics = fpp_errors::ConsoleEmitter::color();
    let mut ctx = fpp_core::CompilerContext::new(&mut diagnostics);
    let out = fpp_core::run(&mut ctx, compiler_main);

    if diagnostics.has_errors() {
        exit(1)
    }

    println!("{}", out);
}
