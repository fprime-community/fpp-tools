use crate::global_state::{GlobalState, TranslationUnitCache, Workspace};
use fpp_analysis::Analysis;
use fpp_ast::MutVisitor;
use fpp_core::{CompilerContext, Diagnostic, GarbageCollectionSet, Level, SourceFile, Spanned};
use lsp_types::Uri;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

use fpp_parser::ResolveIncludes;
use ignore::WalkBuilder;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::mem;
use std::str::FromStr;
use std::time::Instant;

#[derive(Debug)]
pub enum Task {
    Response(lsp_server::Response),
    // Retry(lsp_server::Request),
    ReloadWorkspace,
    LoadLocsFile(Uri),
    LoadFullWorkspace,
    /// The VFS indicated a file changed, we need to reprocess it in the analysis
    /// This may trigger 0+ 'Reprocess' tasks
    Update(Uri),
    /// This source file has changed and must be re-analyzed
    /// Read the contents from the VFS, parse it, resolve includes, and schedule an analysis
    Reprocess(SourceFile),
    /// One or more translation units have changed and semantic analysis is now out of date
    /// Rerun semantic analysis incorporating all the currently cached translation units
    Analysis,
}

impl Display for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Task::Response(r) => match &r.error {
                None => f.write_fmt(format_args!("Response {{ id = {} }}", r.id)),
                Some(err) => f.write_fmt(format_args!(
                    "Response {{ id = {}, err = {:?} }}",
                    r.id, err
                )),
            },
            Task::ReloadWorkspace => f.write_str("ReloadWorkspace"),
            Task::LoadLocsFile(uri) => {
                f.write_fmt(format_args!("LoadLocsFile {{ uri = {} }}", uri.as_str()))
            }
            Task::LoadFullWorkspace => f.write_str("LoadFullWorkspace"),
            Task::Update(uri) => f.write_fmt(format_args!("Update {{ uri = {} }}", uri.as_str())),
            Task::Reprocess(_) => f.write_str("Reprocess"),
            Task::Analysis => f.write_str("Analysis"),
        }
    }
}

impl GlobalState {
    fn new_translation_unit_cache(&self, uri: &str) -> anyhow::Result<TranslationUnitCache> {
        // Read the file from VFS before starting garbage collection so a read
        // error returns without an open GC context.
        let content = self.vfs.read(uri)?;

        GarbageCollectionSet::start();
        self.diagnostics.start_garbage_collection();

        // Produce the initial AST
        let file = SourceFile::new(uri, content);

        let mut ast = fpp_ast::TransUnit(fpp_parser::parse(
            file,
            |p: &mut fpp_parser::Parser| p.module_members(),
            None,
        ));

        let mut include_context_map = Default::default();
        let _ =
            ResolveIncludes::new(&self.vfs).visit_trans_unit(&mut include_context_map, &mut ast);

        // Synthesize the `State` enum for each state machine. This must run once,
        // here, so it is baked into the cached AST rather than re-applied on every
        // analysis pass.
        fpp_analysis::add_state_enums(&mut ast);

        tracing::debug!(file = %file, file_dbg = ?file, "computed translation unit cache");

        Ok(TranslationUnitCache {
            file,
            ast,
            include_context_map,
            gc: GarbageCollectionSet::finish(),
            diagnostics: self.diagnostics.finish_garbage_collection(),
        })
    }

    /// Push the current diagnostic set to the client for every affected URI.
    ///
    /// Diagnostics are push-based: rather than let the client pull (which could
    /// race the debounced analysis and read a stale store), we send the
    /// authoritative set ourselves once analysis finishes. Any URI that had
    /// diagnostics on the previous push but is now clean receives an empty set so
    /// the editor clears it.
    fn publish_diagnostics(&mut self) {
        let current = self.diagnostics.all();

        // Clear any URI that had diagnostics last time but no longer does.
        for uri in &self.published_uris {
            if !current.contains_key(uri)
                && let Ok(parsed) = Uri::from_str(uri)
            {
                self.send_notification::<lsp_types::notification::PublishDiagnostics>(
                    lsp_types::PublishDiagnosticsParams {
                        uri: parsed,
                        diagnostics: Vec::new(),
                        version: None,
                    },
                );
            }
        }

        let mut published = FxHashSet::default();
        for (uri, diagnostics) in current {
            if let Ok(parsed) = Uri::from_str(&uri) {
                self.send_notification::<lsp_types::notification::PublishDiagnostics>(
                    lsp_types::PublishDiagnosticsParams {
                        uri: parsed,
                        diagnostics,
                        version: None,
                    },
                );
                published.insert(uri);
            }
        }

        self.published_uris = published;
    }

    pub fn parent_file(&self, file: SourceFile) -> SourceFile {
        let mut parent = file;
        loop {
            match self.context.file_get(&parent).parent {
                None => return parent,
                Some(p) => {
                    parent = p;
                }
            }
        }
    }

    pub(crate) fn on_task(&mut self, task: Task) {
        let span = tracing::info_span!("", task = %task);
        let _enter = span.enter();

        match task {
            Task::ReloadWorkspace => {
                tracing::info!("reloading workspace");

                // Re-read the `.fpp-lsp` project config so external edits to it are
                // picked up. This is the single source of truth for project setup.
                self.workspace = crate::config::discover(self.workspace_folders.as_deref());

                match self.workspace.clone() {
                    Workspace::None => {
                        tracing::info!("no .fpp-lsp project config found; workspace not indexed");
                    }
                    Workspace::LocsFile(uri) => self.task(Task::LoadLocsFile(uri)),
                    Workspace::Full => self.task(Task::LoadFullWorkspace),
                }
            }
            Task::LoadLocsFile(locs_uri) => {
                tracing::info!("reloading locs file");

                let now = Instant::now();

                // Read the locs file to build a list of files to add to the analysis
                let locs_content = match self.vfs.read(locs_uri.as_str()) {
                    Ok(locs_content) => locs_content,
                    Err(err) => {
                        tracing::warn!(err = ?err, "failed to read locs file during workspace reload");
                        return;
                    }
                };

                // Refresh the context and all caches
                self.diagnostics.clear();
                self.cache = Default::default();
                self.files = Default::default();
                self.analysis = Arc::new(Analysis::new());
                self.workspace = Workspace::LocsFile(locs_uri.clone());

                let vfs = self.vfs.clone();

                let mut progress = self.new_progress("Indexing locs file", 1);

                let mut ctx = CompilerContext::new(self.diagnostics.clone());
                self.cache = fpp_core::run(&mut ctx, || {
                    let mut file_locs = FxHashMap::default();

                    GarbageCollectionSet::start();
                    let locs_tu = fpp_parser::parse(
                        SourceFile::new(locs_uri.as_str(), locs_content),
                        |p| p.module_members(),
                        None,
                    );
                    let locs_gc = GarbageCollectionSet::finish();

                    let files: FxHashSet<String> = locs_tu
                        .into_iter()
                        .filter_map(|loc| match loc {
                            fpp_ast::ModuleMember::SpecLoc(loc) => Some(loc),
                            _ => None,
                        })
                        .filter_map(|loc| {
                            match vfs.resolve_uri_relative_path(locs_uri.as_str(), &loc.file.data) {
                                Ok(file_uri) => {
                                    file_locs.insert(file_uri.clone(), loc);
                                    Some(file_uri)
                                }
                                Err(err) => {
                                    Diagnostic::new(
                                        loc,
                                        Level::Error,
                                        "failed to resolve location specifier",
                                    )
                                    .annotation(err.to_string())
                                    .emit();
                                    None
                                }
                            }
                        })
                        .collect();

                    progress.set_total(files.len());

                    let out = files
                        .into_iter()
                        .filter_map(|file_uri| {
                            let filename = &file_uri
                                [(file_uri.rfind("/").unwrap_or(0) + 1).min(file_uri.len())..];
                            progress.report(filename);

                            tracing::debug!(uri = %file_uri, "processing file from locs");
                            match self.new_translation_unit_cache(&file_uri) {
                                Ok(tu_cache) => Some((tu_cache.file, Arc::new(tu_cache))),
                                Err(err) => {
                                    Diagnostic::new(
                                        file_locs.get(&file_uri).unwrap().span(),
                                        Level::Error,
                                        "failed to process location specifier",
                                    )
                                    .annotation(err.to_string())
                                    .emit();
                                    None
                                }
                            }
                        })
                        .collect();

                    locs_gc.cleanup();
                    out
                });

                // Replace the context and drop the old one
                let _ = mem::replace(&mut self.context, ctx);

                tracing::info!(
                    "finished reparsing workspace in {:.1}s",
                    now.elapsed().as_secs_f64()
                );
                self.task(Task::Analysis);
                progress.finish(None);
            }
            Task::LoadFullWorkspace => {
                let now = Instant::now();

                let mut progress = self.new_progress("Indexing workspace", 1);

                let workspace_folders = match &self.workspace_folders {
                    None => {
                        tracing::warn!("ignoring load workspace index without a workspace loaded");
                        return;
                    }
                    Some(f) => f,
                };

                tracing::info!("scanning workspace for FPP files");

                // WalkBuilder automatically respects .gitignore rules by default
                // Scan for all the .fpp files in the workspace
                let mut files = vec![];
                for workspace_file in workspace_folders {
                    for result in WalkBuilder::new(workspace_file.uri.path().as_str()).build() {
                        match result {
                            Ok(entry) => {
                                let path = entry.path();

                                // Check if the entry is a file and matches the extension filter
                                if path.is_file()
                                    && path.extension().is_some_and(|ext| OsStr::new("fpp") == ext)
                                {
                                    match crate::uri::from_file_path(path) {
                                        Ok(uri_str) => match Uri::from_str(&uri_str) {
                                            Ok(uri) => {
                                                files.push(uri);
                                            }
                                            Err(err) => {
                                                tracing::warn!(context = "load full workspace", err = ?err, "failed to convert file URI into Uri");
                                            }
                                        },
                                        Err(()) => {
                                            tracing::warn!(context = "load full workspace", path = ?path, "convert file path into URI");
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                tracing::warn!(context = "load full workspace", err = ?err, "failed to walk directory");
                            }
                        }
                    }
                }

                tracing::info!("found {} FPP files", files.len());
                progress.set_total(files.len());

                // Refresh the context and all caches
                self.diagnostics.clear();
                self.cache = Default::default();
                self.files = Default::default();
                self.analysis = Arc::new(Analysis::new());
                self.workspace = Workspace::Full;

                let mut ctx = CompilerContext::new(self.diagnostics.clone());
                let cache = fpp_core::run(&mut ctx, || {
                    let mut cache = FxHashMap::default();
                    for file in files {
                        let path = file.path().as_str();
                        let filename = &path[(path.rfind("/").unwrap_or(0) + 1).min(path.len())..];
                        progress.report(filename);

                        match self.new_translation_unit_cache(file.as_str()) {
                            Ok(tu_cache) => {
                                cache.insert(tu_cache.file, Arc::new(tu_cache));
                            }
                            Err(err) => {
                                tracing::error!(file_uri = %file.as_str(), err = ?err, "failed to process file in workspace");
                            }
                        }
                    }

                    cache
                });

                // Replace the context and drop the old one
                let _ = mem::replace(&mut self.context, ctx);

                tracing::info!(
                    "finished reparsing workspace in {:.1}s",
                    now.elapsed().as_secs_f64()
                );
                progress.finish(None);

                self.cache = cache;
                self.task(Task::Analysis);
            }
            Task::Response(response) => self.respond(response),
            Task::Update(uri) => {
                tracing::info!("updating file");

                // Check if this file is the locs file
                if self.workspace == Workspace::LocsFile(uri.clone()) {
                    tracing::info!("workspace locs has updated, refreshing workspace");
                    self.task(Task::ReloadWorkspace);
                    return;
                }

                // Check if this file is currently part of the compiler context
                match self.files.get(uri.as_str()) {
                    None => {
                        tracing::warn!("not part of the compiler context, ignoring analysis");
                    }
                    Some(files) => {
                        // This file is added in one or more ways to the compiler analysis
                        // Select the top level parent that needs to be updated
                        let update_set: FxHashSet<SourceFile> =
                            files.iter().map(|file| self.parent_file(*file)).collect();

                        for file in update_set {
                            self.task(Task::Reprocess(file))
                        }
                    }
                }
            }
            Task::Reprocess(source_file) => {
                let old_cache = self.cache.remove(&source_file).unwrap();
                self.diagnostics
                    .cleanup_garbage_collection(&old_cache.diagnostics);

                let mut ctx = mem::replace(
                    &mut self.context,
                    CompilerContext::new(self.diagnostics.clone()),
                );
                let new_cache = match fpp_core::run(&mut ctx, || {
                    assert!(source_file.parent().is_none());
                    assert_eq!(old_cache.file, source_file);
                    let uri = source_file.uri();

                    tracing::info!(source_file = ?source_file, "reprocessing source file");

                    // Clean up the old file cache in the compiler context
                    old_cache.gc.cleanup();

                    (
                        self.new_translation_unit_cache(uri.as_str()),
                        uri.as_str().to_string(),
                    )
                }) {
                    (Ok(tu_cache), _) => tu_cache,
                    (Err(err), uri) => {
                        tracing::error!(context = "reprocess file", uri = %uri, err = ?err, "failed to reprocess file");
                        return;
                    }
                };

                let _ = mem::replace(&mut self.context, ctx);
                self.cache.insert(new_cache.file, Arc::new(new_cache));

                // Don't run the expensive full-workspace analysis eagerly. Mark it
                // dirty and let the main loop coalesce a burst of edits into a single
                // debounced `Task::Analysis` once typing settles.
                self.analysis_dirty = true;
            }
            Task::Analysis => {
                // Clear all analysis diagnostics
                self.diagnostics
                    .cleanup_garbage_collection(&self.analysis_diagnostics);
                self.diagnostics.start_garbage_collection();

                let (analysis, files) = fpp_core::run(&mut self.context, || {
                    let mut files = FxHashMap::default();
                    let mut analysis = Analysis::new();

                    for (file, cache) in &self.cache {
                        for (included, include_context) in &cache.include_context_map {
                            analysis
                                .include_context_map
                                .insert(*included, *include_context);
                            let included_uri = included.uri();

                            match files.get_mut(&included_uri) {
                                None => {
                                    files.insert(included_uri, vec![*included]);
                                }
                                Some(v) => {
                                    v.push(*included);
                                }
                            }
                        }

                        let file_uri = file.uri();
                        match files.get_mut(&file_uri) {
                            None => {
                                files.insert(file_uri, vec![*file]);
                            }
                            Some(v) => {
                                v.push(*file);
                            }
                        }
                    }

                    let start = std::time::Instant::now();

                    // Template expansion rewrites the tree, so it runs on a copy:
                    // the cached ASTs are shared with the request handlers, which
                    // walk them by source position. An expansion copy carries the
                    // span of the template body it came from, so leaving it out of
                    // the cache keeps position lookups unambiguous.
                    let mut asts: Vec<fpp_ast::TransUnit> =
                        self.cache.values().map(|v| v.ast.clone()).collect();
                    // The cached units already have their includes resolved and
                    // their state enums added, so the analysis starts from the
                    // steps that follow those in ExpandSyntaxAndEnterSymbols.
                    if fpp_analysis::resolve_templates(&mut analysis, &mut asts).is_continue() {
                        let _ = fpp_analysis::check_semantics(&mut analysis, asts.iter().collect());
                    }
                    tracing::info!(
                        "analyzed {} Translation Units in {:.2?}",
                        self.cache.len(),
                        start.elapsed()
                    );

                    (analysis, files)
                });

                self.analysis_diagnostics = self.diagnostics.finish_garbage_collection();
                self.files = files;
                self.analysis = Arc::new(analysis);

                // Push the fresh diagnostic set to the client. Analysis is debounced,
                // so a pull-based client could otherwise read the store mid-window and
                // show stale results; pushing sends the authoritative set directly.
                self.publish_diagnostics();

                // Semantic highlights are pull-based and resolve colors against
                // `self.analysis`. Edits refresh that snapshot only here, after the
                // debounce, so the tokens the client pulled mid-edit were computed
                // against the stale pre-edit analysis. Ask the client to re-pull now
                // that the authoritative snapshot is in place, otherwise stale
                // highlights persist until an unrelated event triggers another pull.
                self.send_request::<lsp_types::request::SemanticTokensRefresh>((), |_, _| {});
            }
        }
    }
}
