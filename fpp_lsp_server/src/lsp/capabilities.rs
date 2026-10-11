//! Advertises the capabilities of the LSP Server.
use fpp_core::WideEncoding;
use lsp_types::{
    CodeActionKind, CodeActionOptions, CodeActionProviderCapability, CompletionOptions,
    FileOperationFilter, FileOperationPattern, FileOperationPatternKind,
    FileOperationRegistrationOptions, OneOf, PositionEncodingKind, SaveOptions,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    WorkspaceFileOperationsServerCapabilities, WorkspaceFoldersServerCapabilities,
    WorkspaceServerCapabilities,
};

use crate::lsp::semantic_tokens::SemanticTokenKind;

#[derive(Clone, Copy)]
pub enum PositionEncoding {
    Utf8,
    Wide(WideEncoding),
}

pub fn server_capabilities(caps: &ClientCapabilities) -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: match caps.negotiated_encoding() {
            PositionEncoding::Utf8 => Some(PositionEncodingKind::UTF8),
            PositionEncoding::Wide(wide) => match wide {
                WideEncoding::Utf16 => Some(PositionEncodingKind::UTF16),
                WideEncoding::Utf32 => Some(PositionEncodingKind::UTF32),
                _ => Some(PositionEncodingKind::UTF16),
            },
        },
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                will_save: None,
                will_save_wait_until: None,
                save: if caps.did_save_text_document_dynamic_registration() {
                    None
                } else {
                    Some(SaveOptions::default().into())
                },
            },
        )),
        workspace: Some(WorkspaceServerCapabilities {
            workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(OneOf::Left(true)),
            }),
            file_operations: Some(WorkspaceFileOperationsServerCapabilities {
                did_create: None,
                will_create: None,
                did_rename: None,
                will_rename: Some(FileOperationRegistrationOptions {
                    filters: vec![
                        FileOperationFilter {
                            scheme: Some(String::from("file")),
                            pattern: FileOperationPattern {
                                glob: String::from("**/*.fpp"),
                                matches: Some(FileOperationPatternKind::File),
                                options: None,
                            },
                        },
                        FileOperationFilter {
                            scheme: Some(String::from("file")),
                            pattern: FileOperationPattern {
                                glob: String::from("**/*.fppi"),
                                matches: Some(FileOperationPatternKind::File),
                                options: None,
                            },
                        },
                        FileOperationFilter {
                            scheme: Some(String::from("file")),
                            pattern: FileOperationPattern {
                                glob: String::from("**"),
                                matches: Some(FileOperationPatternKind::Folder),
                                options: None,
                            },
                        },
                    ],
                }),
                did_delete: None,
                will_delete: None,
            }),
        }),
        semantic_tokens_provider: Some(
            SemanticTokensOptions {
                legend: SemanticTokensLegend {
                    token_types: SemanticTokenKind::TOKEN_TYPES.into(),
                    token_modifiers: SemanticTokenKind::TOKEN_MODIFIERS.into(),
                },

                full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                range: Some(true),
                work_done_progress_options: Default::default(),
            }
            .into(),
        ),
        // Diagnostics are pushed (textDocument/publishDiagnostics) rather than pulled.
        // Analysis is debounced, so a pull-based client could read the diagnostic
        // store mid-debounce and show stale results; pushing lets the server send the
        // authoritative set once analysis settles. Leaving `diagnostic_provider` unset
        // makes vscode-languageclient fall back to push mode.
        diagnostic_provider: None,
        definition_provider: Some(OneOf::Left(true)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        references_provider: Some(OneOf::Left(true)),
        // document_symbol_provider: Some(OneOf::Left(true)),
        // workspace_symbol_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: None,
            trigger_characters: Some(vec![" ".into(), ".".into(), ":".into(), ">".into()]),
            all_commit_characters: None,
            work_done_progress_options: Default::default(),
            completion_item: None,
        }),
        document_link_provider: Some(lsp_types::DocumentLinkOptions {
            resolve_provider: Some(true),
            work_done_progress_options: Default::default(),
        }),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
            resolve_provider: None,
            work_done_progress_options: Default::default(),
        })),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct ClientCapabilities(lsp_types::ClientCapabilities);

impl ClientCapabilities {
    pub fn new(caps: lsp_types::ClientCapabilities) -> Self {
        Self(caps)
    }

    // #[allow(non_snake_case)]
    // pub fn has_completion_item_resolve_additionalTextEdits(&self) -> bool {
    //     (|| {
    //         Some(
    //             self.0
    //                 .text_document
    //                 .as_ref()?
    //                 .completion
    //                 .as_ref()?
    //                 .completion_item
    //                 .as_ref()?
    //                 .resolve_support
    //                 .as_ref()?
    //                 .properties
    //                 .iter()
    //                 .any(|cap_string| cap_string.as_str() == "additionalTextEdits"),
    //         )
    //     })() == Some(true)
    // }

    // pub fn completion_label_details_support(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .text_document
    //             .as_ref()?
    //             .completion
    //             .as_ref()?
    //             .completion_item
    //             .as_ref()?
    //             .label_details_support
    //     })() == Some(true)
    // }

    // fn completion_item(&self) -> Option<CompletionOptionsCompletionItem> {
    //     Some(CompletionOptionsCompletionItem {
    //         label_details_support: Some(self.completion_label_details_support()),
    //     })
    // }

    // fn code_action_capabilities(&self) -> CodeActionProviderCapability {
    //     self.0
    //         .text_document
    //         .as_ref()
    //         .and_then(|it| it.code_action.as_ref())
    //         .and_then(|it| it.code_action_literal_support.as_ref())
    //         .map_or(CodeActionProviderCapability::Simple(true), |_| {
    //             CodeActionProviderCapability::Options(CodeActionOptions {
    //                 // Advertise support for all built-in CodeActionKinds.
    //                 // Ideally we would base this off of the client capabilities
    //                 // but the client is supposed to fall back gracefully for unknown values.
    //                 code_action_kinds: Some(vec![
    //                     CodeActionKind::EMPTY,
    //                     CodeActionKind::QUICKFIX,
    //                     CodeActionKind::REFACTOR,
    //                     CodeActionKind::REFACTOR_EXTRACT,
    //                     CodeActionKind::REFACTOR_INLINE,
    //                     CodeActionKind::REFACTOR_REWRITE,
    //                 ]),
    //                 resolve_provider: Some(true),
    //                 work_done_progress_options: Default::default(),
    //             })
    //         })
    // }

    pub fn negotiated_encoding(&self) -> PositionEncoding {
        let client_encodings = match &self.0.general {
            Some(general) => general.position_encodings.as_deref().unwrap_or_default(),
            None => &[],
        };

        for enc in client_encodings {
            if enc == &PositionEncodingKind::UTF8 {
                return PositionEncoding::Utf8;
            } else if enc == &PositionEncodingKind::UTF32 {
                return PositionEncoding::Wide(WideEncoding::Utf32);
            }
            // NB: intentionally prefer just about anything else to utf-16.
        }

        PositionEncoding::Wide(WideEncoding::Utf16)
    }

    // pub fn workspace_edit_resource_operations(
    //     &self,
    // ) -> Option<&[lsp_types::ResourceOperationKind]> {
    //     self.0
    //         .workspace
    //         .as_ref()?
    //         .workspace_edit
    //         .as_ref()?
    //         .resource_operations
    //         .as_deref()
    // }

    // pub fn semantics_tokens_augments_syntax_tokens(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .text_document
    //             .as_ref()?
    //             .semantic_tokens
    //             .as_ref()?
    //             .augments_syntax_tokens
    //     })()
    //     .unwrap_or(false)
    // }

    pub fn did_save_text_document_dynamic_registration(&self) -> bool {
        let caps = (|| -> _ { self.0.text_document.as_ref()?.synchronization.clone() })()
            .unwrap_or_default();
        caps.did_save == Some(true) && caps.dynamic_registration == Some(true)
    }

    // pub fn did_change_watched_files_dynamic_registration(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .workspace
    //             .as_ref()?
    //             .did_change_watched_files
    //             .as_ref()?
    //             .dynamic_registration
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn did_change_watched_files_relative_pattern_support(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .workspace
    //             .as_ref()?
    //             .did_change_watched_files
    //             .as_ref()?
    //             .relative_pattern_support
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn location_link(&self) -> bool {
    //     (|| -> _ { self.0.text_document.as_ref()?.definition?.link_support })().unwrap_or_default()
    // }

    // pub fn line_folding_only(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .text_document
    //             .as_ref()?
    //             .folding_range
    //             .as_ref()?
    //             .line_folding_only
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn hierarchical_symbols(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .text_document
    //             .as_ref()?
    //             .document_symbol
    //             .as_ref()?
    //             .hierarchical_document_symbol_support
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn code_action_literals(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .text_document
    //             .as_ref()?
    //             .code_action
    //             .as_ref()?
    //             .code_action_literal_support
    //             .as_ref()
    //     })()
    //     .is_some()
    // }

    // pub fn work_done_progress(&self) -> bool {
    //     (|| -> _ { self.0.window.as_ref()?.work_done_progress })().unwrap_or_default()
    // }

    // pub fn will_rename(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .workspace
    //             .as_ref()?
    //             .file_operations
    //             .as_ref()?
    //             .will_rename
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn change_annotation_support(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .workspace
    //             .as_ref()?
    //             .workspace_edit
    //             .as_ref()?
    //             .change_annotation_support
    //             .as_ref()
    //     })()
    //     .is_some()
    // }

    // pub fn code_action_resolve(&self) -> bool {
    //     (|| -> _ {
    //         Some(
    //             self.0
    //                 .text_document
    //                 .as_ref()?
    //                 .code_action
    //                 .as_ref()?
    //                 .resolve_support
    //                 .as_ref()?
    //                 .properties
    //                 .as_slice(),
    //         )
    //     })()
    //     .unwrap_or_default()
    //     .iter()
    //     .any(|it| it == "edit")
    // }

    // pub fn signature_help_label_offsets(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .text_document
    //             .as_ref()?
    //             .signature_help
    //             .as_ref()?
    //             .signature_information
    //             .as_ref()?
    //             .parameter_information
    //             .as_ref()?
    //             .label_offset_support
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn text_document_diagnostic(&self) -> bool {
    //     (|| -> _ { self.0.text_document.as_ref()?.diagnostic.as_ref() })().is_some()
    // }

    // pub fn text_document_diagnostic_related_document_support(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .text_document
    //             .as_ref()?
    //             .diagnostic
    //             .as_ref()?
    //             .related_document_support
    //     })() == Some(true)
    // }

    // pub fn completion_snippet(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .text_document
    //             .as_ref()?
    //             .completion
    //             .as_ref()?
    //             .completion_item
    //             .as_ref()?
    //             .snippet_support
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn semantic_tokens_refresh(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .workspace
    //             .as_ref()?
    //             .semantic_tokens
    //             .as_ref()?
    //             .refresh_support
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn code_lens_refresh(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .workspace
    //             .as_ref()?
    //             .code_lens
    //             .as_ref()?
    //             .refresh_support
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn inlay_hints_refresh(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .workspace
    //             .as_ref()?
    //             .inlay_hint
    //             .as_ref()?
    //             .refresh_support
    //     })()
    //     .unwrap_or_default()
    // }

    // Diagnostics are now pushed, not pulled, so the client's pull-refresh support
    // is no longer consulted. Kept for reference in case pull mode returns.
    // pub fn diagnostics_refresh(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .workspace
    //             .as_ref()?
    //             .diagnostic
    //             .as_ref()?
    //             .refresh_support
    //     })()
    //     .unwrap_or_default()
    // }

    // pub fn inlay_hint_resolve_support_properties(&self) -> FxHashSet<&str> {
    //     self.0
    //         .text_document
    //         .as_ref()
    //         .and_then(|text| text.inlay_hint.as_ref())
    //         .and_then(|inlay_hint_caps| inlay_hint_caps.resolve_support.as_ref())
    //         .map(|inlay_resolve| inlay_resolve.properties.iter())
    //         .into_iter()
    //         .flatten()
    //         .map(|s| s.as_str())
    //         .collect()
    // }

    // pub fn completion_resolve_support_properties(&self) -> FxHashSet<&str> {
    //     self.0
    //         .text_document
    //         .as_ref()
    //         .and_then(|text| text.completion.as_ref())
    //         .and_then(|completion_caps| completion_caps.completion_item.as_ref())
    //         .and_then(|completion_item_caps| completion_item_caps.resolve_support.as_ref())
    //         .map(|resolve_support| resolve_support.properties.iter())
    //         .into_iter()
    //         .flatten()
    //         .map(|s| s.as_str())
    //         .collect()
    // }

    // pub fn hover_markdown_support(&self) -> bool {
    //     (|| -> _ {
    //         Some(
    //             self.0
    //                 .text_document
    //                 .as_ref()?
    //                 .hover
    //                 .as_ref()?
    //                 .content_format
    //                 .as_ref()?
    //                 .as_slice(),
    //         )
    //     })()
    //     .unwrap_or_default()
    //     .contains(&lsp_types::MarkupKind::Markdown)
    // }

    // pub fn insert_replace_support(&self) -> bool {
    //     (|| -> _ {
    //         self.0
    //             .text_document
    //             .as_ref()?
    //             .completion
    //             .as_ref()?
    //             .completion_item
    //             .as_ref()?
    //             .insert_replace_support
    //     })()
    //     .unwrap_or_default()
    // }
}
