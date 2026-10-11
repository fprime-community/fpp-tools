use crate::analysis::Task;
use crate::dispatcher::RequestDispatcher;
use crate::global_state::GlobalState;
use crate::{handlers, lsp_ext};
use lsp_server::Request;

impl GlobalState {
    /// Handles a request.
    pub(crate) fn on_request(&mut self, req: Request) {
        let mut dispatcher = RequestDispatcher {
            req: Some(req),
            global_state: self,
        };

        use lsp_types::request as lsp_request;

        #[rustfmt::skip]
        dispatcher
            .on_mut::<lsp_request::Shutdown>(|s, ()| {
                s.shutdown_requested = true;
                Ok(())
            })
            // Request handlers that must run on the main thread
            // because they mutate GlobalState:
            .on_run_task::<lsp_ext::ReloadWorkspace>(|_| Ok(Task::ReloadWorkspace))
            .on::<lsp_ext::Diagram>(handlers::handle_diagram)
            .on::<lsp_ext::DiagramElements>(handlers::handle_diagram_elements)
            // .on_sync::<lsp_request::SelectionRangeRequest>(handlers::handle_selection_range)
            .on::<lsp_request::Completion>(handlers::handle_completion)
            // .on::<lsp_request::ResolveCompletionItem>(handlers::handle_completion_resolve)
            .on_mut::<lsp_request::SemanticTokensFullRequest>(handlers::handle_semantic_tokens_full)
            .on_mut::<lsp_request::SemanticTokensFullDeltaRequest>(handlers::handle_semantic_tokens_full_delta)
            .on::<lsp_request::SemanticTokensRangeRequest>(handlers::handle_semantic_tokens_range)
            .on_mut::<lsp_request::DocumentDiagnosticRequest>(handlers::handle_document_diagnostics)
            .on::<lsp_request::DocumentLinkRequest>(handlers::handle_document_link_request)
            .on::<lsp_request::DocumentLinkResolve>(handlers::handle_document_link_resolve)
            // .on::<lsp_request::DocumentSymbolRequest>(handlers::handle_document_symbol)
            // .on::<lsp_request::FoldingRangeRequest>(handlers::handle_folding_range)
            // .on::<lsp_request::SignatureHelpRequest>(handlers::handle_signature_help)
            // .on::<lsp_request::WillRenameFiles>(handlers::handle_will_rename_files)
            .on::<lsp_request::GotoDefinition>(handlers::handle_goto_definition)
            .on::<lsp_request::HoverRequest>(handlers::handle_hover)
            // .on::<lsp_request::GotoDeclaration>(handlers::handle_goto_declaration)
            // .on::<lsp_request::GotoImplementation>(handlers::handle_goto_implementation)
            // .on::<lsp_request::InlayHintRequest>(handlers::handle_inlay_hints)
            // .on_identity::<lsp_request::InlayHintResolveRequest, _>(handlers::handle_inlay_hints_resolve)
            // .on::<lsp_request::CodeLensRequest>(handlers::handle_code_lens)
            // .on_identity::<NO_RETRY, lsp_request::CodeLensResolve, _>(handlers::handle_code_lens_resolve)
            // .on::<lsp_request::PrepareRenameRequest>(handlers::handle_prepare_rename)
            // .on::<lsp_request::Rename>(handlers::handle_rename)
            .on::<lsp_request::CodeActionRequest>(handlers::handle_code_action)
            .on::<lsp_request::References>(handlers::handle_references)
            .on::<lsp_request::Formatting>(handlers::handle_formatting)
            .on::<lsp_request::RangeFormatting>(handlers::handle_range_formatting)
            .finish();
    }
}
