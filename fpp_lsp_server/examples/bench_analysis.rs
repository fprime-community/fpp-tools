//! Standalone benchmark that mirrors the LSP's locs-based indexing + semantic
//! analysis so we can profile `check_semantics` per-pass outside the editor.
//!
//! Usage:
//!   cargo run --release -p fpp_lsp_server --example bench_analysis -- <path/to/locs.fpp>

use fpp_analysis::Analysis;
use fpp_ast::MutVisitor;
use fpp_core::{CompilerContext, FileReader, SourceFile};
use fpp_fs::FsReader;
use std::time::Instant;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let locs_path = std::env::args()
        .nth(1)
        .expect("usage: bench_analysis <path/to/locs.fpp>");

    let iters: usize = std::env::var("ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let mut diagnostics = fpp_errors::ConsoleEmitter::color();
    let mut ctx = CompilerContext::new(&mut diagnostics);

    fpp_core::run(&mut ctx, || {
        let reader = FsReader {};

        // Parse the locs file to discover the set of translation units.
        let locs_content = reader.read(&locs_path).expect("read locs file");
        let locs_tu = fpp_parser::parse(
            SourceFile::new(&locs_path, locs_content),
            |p| p.module_members(),
            None,
        );

        let files: Vec<String> = locs_tu
            .into_iter()
            .filter_map(|m| match m {
                fpp_ast::ModuleMember::SpecLoc(loc) => {
                    reader.resolve_from_locs(&locs_path, &loc.file.data)
                }
                _ => None,
            })
            .collect();

        eprintln!("discovered {} translation units", files.len());

        // Parse + resolve includes for each TU (this is the LSP's per-file cache
        // build; we time it separately from analysis).
        let parse_start = Instant::now();
        let mut asts: Vec<fpp_ast::TransUnit> = Vec::with_capacity(files.len());
        for file_path in &files {
            let content = match reader.read(file_path) {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("skip {file_path}: {err}");
                    continue;
                }
            };
            let mut ast = fpp_ast::TransUnit(fpp_parser::parse(
                SourceFile::new(file_path, content),
                |p: &mut fpp_parser::Parser| p.module_members(),
                None,
            ));
            let mut include_context_map = Default::default();
            let _ = fpp_parser::ResolveIncludes::new(FsReader {})
                .visit_trans_unit(&mut include_context_map, &mut ast);
            fpp_analysis::add_state_enums(&mut ast);
            asts.push(ast);
        }
        eprintln!(
            "parse + resolve_includes: {:.3}s ({} TUs)",
            parse_start.elapsed().as_secs_f64(),
            asts.len()
        );

        for i in 0..iters {
            let mut analysis = Analysis::new();
            let mut asts = asts.clone();
            let start = Instant::now();
            if fpp_analysis::resolve_templates(&mut analysis, &mut asts).is_continue() {
                let _ = fpp_analysis::check_semantics(&mut analysis, asts.iter().collect());
            }
            eprintln!(
                "iter {i}: check_semantics total = {:.3}s",
                start.elapsed().as_secs_f64()
            );
        }
    });
}

trait LocsResolve {
    fn resolve_from_locs(&self, locs_path: &str, rel: &str) -> Option<String>;
}

impl LocsResolve for FsReader {
    fn resolve_from_locs(&self, locs_path: &str, rel: &str) -> Option<String> {
        let dir = std::path::Path::new(locs_path).parent()?;
        let joined = dir.join(rel);
        let canon = joined.canonicalize().ok()?;
        Some(canon.to_str()?.to_string())
    }
}
