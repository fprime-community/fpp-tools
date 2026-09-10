//! The unified `fpp_python` declaration generator.
//!
//! Run from the workspace root (or anywhere — every default path is anchored to
//! this crate's location, not the CWD):
//!   cargo run -p fpp_python_bindgen
//!
//! One tool emits BOTH checked-in declaration files from the in-workspace
//! compiler crates:
//!   * `fpp_python/src/ast/defs.rs` — a `fpp_ast_bindings!` mirror of the
//!     `fpp_ast` grammar, parsed from source with `syn` (see [`partition`] +
//!     [`ast_emit`]).
//!   * `fpp_python/src/sem/defs.rs` — a `fpp_sem_bindings!` mirror of the
//!     `fpp_analysis` semantic layer, reflected from nightly rustdoc JSON (see
//!     [`sem`]).
//!
//! The two share ONE `fpp_ast` partition: the semantic generator resolves every
//! `fpp_ast::X` reference against the REAL grammar classification (nodes / unions
//! / kind-enums / leaves) rather than a `Def*/Spec*` name prefix, and the set of
//! entity-shadowed AST nodes is DERIVED — `node_names ∩ resolved-semantic-names`
//! — instead of a hand-maintained list.
//!
//! # Phases (default full run)
//!
//! A. build the AST partition from `fpp_ast` source (always; both emitters need it).
//! B. reflect `fpp_analysis` from rustdoc JSON; the `fpp_ast` arm resolves against
//!    the partition, and every referenced leaf is cross-fed back to it.
//! C. resolve the semantic Python names.
//! D. derive the entity-shadowed AST nodes and install them on the partition.
//! E. normalize: rewrite `astdef(x)` → `skip` for each shadowed `x`.
//! F. emit `ast/defs.rs`.
//! G. emit `sem/defs.rs`.
//!
//! `--only ast` runs A + F (no nightly), reusing the committed `shadowed {…}` line.
//! `--only sem` runs A + B..E + G, validating that every `leaf(crate::ast::X)` it
//! emits already has a Python-enum mirror in the committed `ast/defs.rs`.
//!
//! # Toolchain
//!
//! Phase B needs a nightly `rustdoc`, pinned exactly by
//! `fpp_python_bindgen/nightly-toolchain` ([`NIGHTLY_TOOLCHAIN`]) because the
//! rustdoc JSON schema is unstable. Install it once with
//! `rustup toolchain install $(cat fpp_python_bindgen/nightly-toolchain)`; set
//! `FPP_BINDGEN_TOOLCHAIN` to override.

// `name` fields on the partition def structs are retained for debugging.
#![allow(dead_code)]

mod ast_emit;
mod partition;
mod sem;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use rustdoc_types::Crate;

fn main() {
    let cfg = resolve_config();
    match cfg.only {
        Some(Only::Ast) => run_ast_only(&cfg),
        Some(Only::Sem) => run_sem_only(&cfg),
        None => run_full(&cfg),
    }
}

// ---------------------------------------------------------------------------
// Phase orchestration
// ---------------------------------------------------------------------------

/// Default full run: A → G (nightly rustdoc).
fn run_full(cfg: &Config) {
    // A: the shared AST partition (also runs the string-leaf/classification asserts).
    let mut part = partition::build(&cfg.fpp_src);

    // B: reflect `fpp_analysis`; the `fpp_ast` arm consults the partition.
    let krate = load_krate(cfg);
    let mut ctx = sem::prepare(&krate, part.ast_class());
    let mut reflected = sem::reflect(&mut ctx);
    sem::assert_special_union_handles(&ctx, &reflected);
    sem::assert_core_unions_present(&ctx, &reflected);
    // Cross-feed: every leaf the sem layer references gets a mirror in `ast/defs.rs`.
    for leaf in ctx.used_ast_leaves() {
        part.register_used_leaf(leaf.clone());
    }

    // C: resolve the semantic Python names.
    let names = sem::resolve_names(&ctx, &reflected);

    // D: derive the entity-shadowed AST nodes from the RESOLVED semantic names.
    let sem_names = names.all_python_names();
    let shadowed: BTreeSet<String> = part
        .node_names()
        .intersection(&sem_names)
        .cloned()
        .collect();
    eprintln!(
        "bindgen: derived entity-shadowed nodes = {}",
        fmt_set(&shadowed)
    );
    part.set_shadowed(shadowed.clone());

    // E: normalize the reflected model (shadowed astdef → skip).
    for line in sem::apply_shadow(&mut reflected, &names, &shadowed) {
        eprintln!("{line}");
    }

    // F: emit the AST declaration.
    ast_emit::write_defs(&cfg.ast_out, &part, &cfg.ast_version);
    summarize_ast(cfg, &part);

    // G: emit the semantic declaration.
    let mut skips = Vec::new();
    sem::warn_identityless_map_keys(&ctx, &reflected, &names, &mut skips);
    let text = sem::emit(&ctx, &reflected, &names, &cfg.sem_version, &mut skips);
    sem::write_out(&cfg.sem_out, &text);
    finish_sem(cfg, &ctx, &reflected, skips);
}

/// `--only ast`: A + F. No nightly; the shadowed set is reused from the committed
/// declaration (only the full run — with the sem reflection — can derive it).
fn run_ast_only(cfg: &Config) {
    let mut part = partition::build(&cfg.fpp_src);
    // Preserve leaves cross-fed from the semantic layer on the last full run: this
    // path does not reflect `fpp_analysis`, so it cannot re-derive a leaf that only
    // the sem side references (no `fpp_ast` field uses it). Reuse the committed
    // `leaves {…}` block so such a Python-enum mirror is not silently dropped.
    if let Some(committed) = partition::read_committed_leaves(&cfg.ast_out) {
        for leaf in committed {
            part.register_used_leaf(leaf);
        }
    }
    let shadowed = partition::read_committed_shadowed(&cfg.ast_out).unwrap_or_else(|| {
        panic!(
            "--only ast needs the `shadowed {{…}}` line from the committed {} to preserve \
             it, but none was found — run the full bindgen (it derives the set from the \
             semantic layer)",
            cfg.ast_out.display()
        )
    });
    eprintln!(
        "bindgen: reusing committed entity-shadowed nodes = {}",
        fmt_set(&shadowed)
    );
    part.set_shadowed(shadowed);
    ast_emit::write_defs(&cfg.ast_out, &part, &cfg.ast_version);
    summarize_ast(cfg, &part);
}

/// `--only sem`: A + B..E + G (skip F). Guards that every `leaf(crate::ast::X)`
/// the semantic file emits already has a mirror in the committed `ast/defs.rs`.
fn run_sem_only(cfg: &Config) {
    let part = partition::build(&cfg.fpp_src);
    let krate = load_krate(cfg);
    let mut ctx = sem::prepare(&krate, part.ast_class());
    let mut reflected = sem::reflect(&mut ctx);
    sem::assert_special_union_handles(&ctx, &reflected);
    sem::assert_core_unions_present(&ctx, &reflected);

    // Guard: without emitting `ast/defs.rs`, verify the committed one already
    // provides a Python-enum mirror for every leaf the sem layer references.
    let committed_leaves = partition::read_committed_leaves(&cfg.ast_out).unwrap_or_else(|| {
        panic!(
            "--only sem needs the committed {} `leaves {{…}}` block to validate leaf \
             references, but none was found — run the full bindgen",
            cfg.ast_out.display()
        )
    });
    for x in ctx.used_ast_leaves() {
        assert!(
            committed_leaves.contains(x),
            "sem references leaf(crate::ast::{x}) but the committed {} has no Python-enum \
             mirror for it — run the full bindgen",
            cfg.ast_out.display()
        );
    }

    let names = sem::resolve_names(&ctx, &reflected);
    let sem_names = names.all_python_names();
    let shadowed: BTreeSet<String> = part
        .node_names()
        .intersection(&sem_names)
        .cloned()
        .collect();
    eprintln!(
        "bindgen: derived entity-shadowed nodes = {}",
        fmt_set(&shadowed)
    );
    for line in sem::apply_shadow(&mut reflected, &names, &shadowed) {
        eprintln!("{line}");
    }

    let mut skips = Vec::new();
    sem::warn_identityless_map_keys(&ctx, &reflected, &names, &mut skips);
    let text = sem::emit(&ctx, &reflected, &names, &cfg.sem_version, &mut skips);
    sem::write_out(&cfg.sem_out, &text);
    finish_sem(cfg, &ctx, &reflected, skips);
}

/// Fold in the reflection notes (field/method-collision drops, param/return-dropped
/// methods), dedup, log, and print the semantic summary line.
fn finish_sem(cfg: &Config, ctx: &sem::Ctx, reflected: &sem::Reflected, mut skips: Vec<String>) {
    skips.extend(ctx.skips().iter().cloned());
    let mut seen = BTreeSet::new();
    skips.retain(|s| seen.insert(s.clone()));
    for s in &skips {
        eprintln!("{s}");
    }
    eprintln!(
        "bindgen: fpp_analysis v{} -> {} ({} unions, {} payloads, {} entities, {} leaf_enums; {} skips)",
        cfg.sem_version,
        cfg.sem_out.display(),
        reflected.unions.len(),
        reflected.payloads.len(),
        reflected.entities.len(),
        reflected.leaf_enums.len(),
        skips.len(),
    );
}

fn summarize_ast(cfg: &Config, part: &partition::Registry) {
    eprintln!(
        "bindgen: fpp_ast v{} (src {}) -> {} ({} nodes, {} unions, {} kinds, {} leaves)",
        cfg.ast_version,
        cfg.fpp_src.display(),
        cfg.ast_out.display(),
        part.node_structs.len(),
        part.unions.len(),
        part.kinds.len(),
        part.used_leaves.len(),
    );
}

fn fmt_set(set: &BTreeSet<String>) -> String {
    let items: Vec<&str> = set.iter().map(String::as_str).collect();
    format!("{{{}}}", items.join(", "))
}

/// The nightly toolchain rustdoc JSON is generated with, single-sourced from
/// `fpp_python_bindgen/nightly-toolchain` so the CI job that installs it and this
/// tool that invokes it cannot disagree.
///
/// The rustdoc JSON schema is unstable and bumps `format_version` on its own
/// cadence, so a floating `nightly` would break `sem/defs.rs` generation the next
/// time it moved — and, worse, could silently reshape the output and trip the
/// drift check. The pin is exact for the same reason `rustdoc-types` is: the two
/// must be bumped together (see [`load_krate`]).
const NIGHTLY_TOOLCHAIN: &str = include_str!("../nightly-toolchain");

/// The pinned nightly, or the `FPP_BINDGEN_TOOLCHAIN` override.
fn nightly_toolchain() -> String {
    std::env::var("FPP_BINDGEN_TOOLCHAIN").unwrap_or_else(|_| NIGHTLY_TOOLCHAIN.trim().to_string())
}

/// Load + parse the rustdoc JSON, asserting its schema version. With
/// `--rustdoc-json` the file is read directly; otherwise the pinned nightly
/// rustdoc is invoked via the `rustdoc-json` crate over the resolved
/// `fpp_analysis` manifest.
fn load_krate(cfg: &Config) -> Crate {
    let toolchain = nightly_toolchain();
    let json = match &cfg.rustdoc_json {
        Some(p) => std::fs::read_to_string(p)
            .unwrap_or_else(|e| panic!("read rustdoc JSON {}: {e}", p.display())),
        None => {
            let path = rustdoc_json::Builder::default()
                .toolchain(&toolchain)
                .manifest_path(&cfg.manifest)
                .document_private_items(true)
                .build()
                .unwrap_or_else(|e| {
                    panic!(
                        "rustdoc-json build failed with toolchain `{toolchain}`: {e}\n\
                         install it with `rustup toolchain install {toolchain}`"
                    )
                });
            std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read rustdoc JSON {}: {e}", path.display()))
        }
    };
    let krate: Crate =
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("parse rustdoc JSON: {e}"));
    assert_eq!(
        krate.format_version,
        rustdoc_types::FORMAT_VERSION,
        "rustdoc JSON format_version {} != rustdoc_types::FORMAT_VERSION {} — the emitting \
         toolchain and the schema model disagree.\n\
         Expected the pinned nightly (`fpp_python_bindgen/nightly-toolchain`: {}); to move to a \
         newer nightly, bump that file AND the `rustdoc-types` pin in \
         `fpp_python_bindgen/Cargo.toml` together, then regenerate.",
        krate.format_version,
        rustdoc_types::FORMAT_VERSION,
        NIGHTLY_TOOLCHAIN.trim(),
    );
    krate
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

enum Only {
    Ast,
    Sem,
}

/// Resolved codegen inputs/outputs (CLI overrides layered over `cargo metadata`).
struct Config {
    /// `fpp_ast/src` directory (parsed with `syn`).
    fpp_src: PathBuf,
    /// Pre-built rustdoc JSON; if `None`, nightly rustdoc is invoked.
    rustdoc_json: Option<PathBuf>,
    /// `fpp_analysis/Cargo.toml` (the rustdoc-json build target).
    manifest: PathBuf,
    ast_out: PathBuf,
    sem_out: PathBuf,
    only: Option<Only>,
    /// Version stamped into `ast/defs.rs` (the resolved `fpp_ast` version).
    ast_version: String,
    /// Version stamped into `sem/defs.rs` (the resolved `fpp_analysis` version).
    sem_version: String,
}

fn resolve_config() -> Config {
    let mut cli_fpp_src: Option<PathBuf> = None;
    let mut cli_rustdoc_json: Option<PathBuf> = None;
    let mut cli_manifest: Option<PathBuf> = None;
    let mut cli_ast_out: Option<PathBuf> = None;
    let mut cli_sem_out: Option<PathBuf> = None;
    let mut cli_only: Option<String> = None;
    let mut cli_version: Option<String> = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--fpp-src" => cli_fpp_src = it.next().map(PathBuf::from),
            "--rustdoc-json" => cli_rustdoc_json = it.next().map(PathBuf::from),
            "--manifest" => cli_manifest = it.next().map(PathBuf::from),
            "--ast-out" => cli_ast_out = it.next().map(PathBuf::from),
            "--sem-out" => cli_sem_out = it.next().map(PathBuf::from),
            "--only" => cli_only = it.next(),
            "--version" => cli_version = it.next(),
            other => eprintln!("bindgen: ignoring unrecognized arg `{other}`"),
        }
    }

    let only = match cli_only.as_deref() {
        Some("ast") => Some(Only::Ast),
        Some("sem") => Some(Only::Sem),
        Some(o) => panic!("--only expects `ast` or `sem`, got `{o}`"),
        None => None,
    };

    let bindgen_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = bindgen_dir.parent().unwrap_or(bindgen_dir);
    let out_crate = workspace.join("fpp_python");

    let meta = cargo_metadata();
    // `--version` overrides both header stamps; otherwise each is the resolved
    // upstream-crate version (so a dependency bump trips the codegen-drift check).
    let ast_version = cli_version
        .clone()
        .or(meta.fpp_ast_version)
        .unwrap_or_else(|| "unknown".to_string());
    let sem_version = cli_version
        .or(meta.fpp_analysis_version)
        .unwrap_or_else(|| "unknown".to_string());

    Config {
        fpp_src: cli_fpp_src
            .or(meta.fpp_ast_src)
            .unwrap_or_else(|| workspace.join("fpp_ast/src")),
        rustdoc_json: cli_rustdoc_json,
        manifest: cli_manifest
            .or(meta.fpp_analysis_manifest)
            .unwrap_or_else(|| workspace.join("fpp_analysis/Cargo.toml")),
        ast_out: cli_ast_out.unwrap_or_else(|| out_crate.join("src/ast/defs.rs")),
        sem_out: cli_sem_out.unwrap_or_else(|| out_crate.join("src/sem/defs.rs")),
        only,
        ast_version,
        sem_version,
    }
}

#[derive(Default)]
struct Meta {
    fpp_ast_src: Option<PathBuf>,
    fpp_ast_version: Option<String>,
    fpp_analysis_manifest: Option<PathBuf>,
    fpp_analysis_version: Option<String>,
}

/// Resolve BOTH `fpp_ast` (src dir + version) and `fpp_analysis` (manifest +
/// version) from a single `cargo metadata` call. Best-effort: any failure leaves
/// the corresponding fields `None` so the caller falls back to defaults / CLI.
fn cargo_metadata() -> Meta {
    let mut meta = Meta::default();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = match Command::new(cargo)
        .args(["metadata", "--format-version", "1"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return meta,
    };
    let val: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return meta,
    };
    let Some(packages) = val.get("packages").and_then(|p| p.as_array()) else {
        return meta;
    };
    for pkg in packages {
        let name = pkg.get("name").and_then(|n| n.as_str());
        let version = pkg
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let manifest = pkg.get("manifest_path").and_then(|m| m.as_str());
        match name {
            Some("fpp_ast") => {
                meta.fpp_ast_version = version;
                // manifest_path is `.../fpp_ast/Cargo.toml`; the sources live in `src/`.
                meta.fpp_ast_src = manifest.map(|m| {
                    let mut pb = PathBuf::from(m);
                    pb.pop();
                    pb.push("src");
                    pb
                });
            }
            Some("fpp_analysis") => {
                meta.fpp_analysis_version = version;
                meta.fpp_analysis_manifest = manifest.map(PathBuf::from);
            }
            _ => {}
        }
    }
    meta
}
