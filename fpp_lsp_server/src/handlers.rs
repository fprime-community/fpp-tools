use crate::global_state::{GlobalState, Task};
use crate::lsp;
use crate::lsp::utils::semantic_token_delta;
use crate::lsp_ext::UriRequest;
use crate::util::{
    completion_items_for_port_instance, completion_items_for_port_match,
    completion_items_for_postfix_expr, completion_items_for_qual_ident, completion_items_for_sm,
    completion_items_for_sm_state, completion_items_in_name_group, hover_for_node,
    hover_for_port_instance, hover_for_sm_symbol, hover_for_symbol, node_to_location,
    nodes_at_offset, port_instance_at_position, port_match_at_position, position_to_offset,
    sm_def_at_position, sm_symbol_at_position, symbol_at_position, symbol_to_completion_item,
};
use anyhow::Result;
use fpp_analysis::semantics::{NameGroup, SymbolInterface};
use fpp_ast::{AstNode, Node};
use fpp_core::{LineCol, LineIndex, SourceFile};
use fpp_lsp_parser::{
    SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TokenAtOffset, VisitorResult,
};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentDiagnosticReportResult, DocumentFormattingParams,
    DocumentLink, DocumentRangeFormattingParams, FileChangeType, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, Location, Position, Range, ReferenceParams,
    SemanticTokensFullDeltaResult, SemanticTokensRangeResult, SemanticTokensResult, TextEdit, Uri,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub fn handle_did_open_text_document(
    state: &mut GlobalState,
    not: DidOpenTextDocumentParams,
) -> Result<()> {
    let uri = not.text_document.uri.clone();
    state.vfs.did_open(not);
    state.task(Task::Update(uri));
    Ok(())
}

pub fn handle_did_change_text_document(
    state: &mut GlobalState,
    not: DidChangeTextDocumentParams,
) -> Result<()> {
    let uri = not.text_document.uri.clone();
    state
        .vfs
        .did_change(not, state.capabilities.negotiated_encoding());
    state.task(Task::Update(uri));
    Ok(())
}

pub fn handle_did_close_text_document(
    state: &mut GlobalState,
    not: DidCloseTextDocumentParams,
) -> Result<()> {
    let uri = not.text_document.uri.clone();

    state.semantic_tokens.remove(&not.text_document.uri);

    state.vfs.did_close(not);
    state.task(Task::Update(uri));

    Ok(())
}

pub fn handle_exit(state: &mut GlobalState, _: ()) -> Result<()> {
    state.shutdown_requested = true;
    Ok(())
}

pub fn handle_did_change_watched_file(
    state: &mut GlobalState,
    params: DidChangeWatchedFilesParams,
) -> Result<()> {
    for file in params.changes {
        // A change to the `.fpp-lsp` project config (created, edited, or removed)
        // re-runs discovery and re-indexes the workspace.
        if file
            .uri
            .path()
            .as_str()
            .ends_with(crate::config::CONFIG_FILE_NAME)
        {
            tracing::info!(uri = %file.uri.as_str(), ".fpp-lsp changed; reloading workspace");
            state.task(Task::ReloadWorkspace);
            continue;
        }

        match file.typ {
            FileChangeType::CHANGED => {
                // This just requires a VFS update
                if state.vfs.update_fs(file.uri.as_str())? {
                    state.task(Task::Update(file.uri));
                }
            }
            FileChangeType::CREATED => {
                // TODO(tumbar)
            }
            FileChangeType::DELETED => {
                // TODO(tumbar)
            }
            t => {
                tracing::warn!(file_change_type = ?t, "dropping invalid file event type");
            }
        }
    }

    Ok(())
}

pub fn handle_diagram(
    state: &GlobalState,
    params: crate::lsp_ext::DiagramParams,
) -> Result<serde_json::Value> {
    // State machines render via Mermaid, not sprotty: return the Mermaid
    // `stateDiagram-v2` source as a JSON string. All other kinds return a
    // sprotty `SModel` JSON object. The client dispatches on the diagram kind.
    if params.kind == crate::lsp_ext::DiagramKind::StateMachine {
        return fpp_diagram::lower_state_machine_to_mermaid(
            &state.analysis,
            &params.name,
            params.transition_action_mode.into(),
        )
        .map(serde_json::Value::String)
        .map_err(|e| anyhow::anyhow!("{e}"));
    }
    fpp_diagram::lower_to_smodel(
        &state.analysis,
        params.kind.into(),
        &params.name,
        params.hide_unused_ports,
        params.transition_action_mode.into(),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

pub fn handle_diagram_elements(
    state: &GlobalState,
    param: UriRequest,
) -> Result<Vec<crate::lsp_ext::DiagramElement>> {
    use crate::lsp_ext::{DiagramElement, DiagramKind};
    use fpp_analysis::semantics::SymbolInterface;

    let uri_str = param.uri.as_str();
    let mut elements: Vec<DiagramElement> = Vec::new();

    // A definition is diagrammable from this document if its defining name node
    // lives in this file. `node_to_location` resolves the node's span to a URI +
    // range, so we filter on that URI.
    let mut push_if_in_doc = |kind: DiagramKind, name: String, display_name: String, name_id| {
        let loc = node_to_location(state, name_id);
        if loc.uri.as_str() == uri_str {
            elements.push(DiagramElement {
                kind,
                name,
                display_name,
                range: loc.range,
            });
        }
    };

    for topology in state.analysis.topology_map.values() {
        push_if_in_doc(
            DiagramKind::Topology,
            topology.qualified_name.clone(),
            topology.symbol.name().data.clone(),
            topology.symbol.name().id(),
        );

        // Each named connection group is independently diagrammable. Anchor its
        // lens at the group's own `connections <name>` location rather than the
        // topology name, so the lenses do not all stack on the topology.
        for graph in &topology.raw_direct_graphs {
            push_if_in_doc(
                DiagramKind::ConnectionGroup,
                format!("{}.{}", topology.qualified_name, graph.name.data),
                graph.name.data.clone(),
                graph.name.id(),
            );
        }
    }

    for component in state.analysis.component_map.values() {
        let qualified_name = state.analysis.get_qualified_name(&component.symbol);
        push_if_in_doc(
            DiagramKind::Component,
            qualified_name,
            component.symbol.name().data.clone(),
            component.symbol.name().id(),
        );
    }

    for sm in state.analysis.state_machine_map.values() {
        // Only internal (defined) state machines have a body to diagram.
        if matches!(
            sm.get_kind(),
            fpp_analysis::semantics::state_machine::Kind::External
        ) {
            continue;
        }
        // A state machine with a blocking analysis error has an incomplete
        // transition graph; skip it so the diagram is not built from partial
        // data. (It is still stored for editor features like completion.)
        if sm.sma.blocking_error {
            continue;
        }
        let symbol = sm.get_symbol();
        let qualified_name = state.analysis.get_qualified_name(&symbol);
        push_if_in_doc(
            DiagramKind::StateMachine,
            qualified_name,
            sm.node.name.data.clone(),
            sm.node.name.id(),
        );
    }

    Ok(elements)
}

pub fn handle_dump_syntax_tree(state: &mut GlobalState, param: UriRequest) -> Result<()> {
    let (_, source_file, parse) = parse_text_document(state, &param.uri)?;

    let parse_kind = source_file
        .and_then(|f| state.analysis.include_context_map.get(&f).cloned())
        .unwrap_or(fpp_parser::IncludeParentKind::Module);

    let entry_kind = match parse_kind {
        fpp_parser::IncludeParentKind::Component => fpp_lsp_parser::TopEntryPoint::Component,
        fpp_parser::IncludeParentKind::Module => fpp_lsp_parser::TopEntryPoint::Module,
        fpp_parser::IncludeParentKind::TlmPacket => fpp_lsp_parser::TopEntryPoint::TlmPacket,
        fpp_parser::IncludeParentKind::TlmPacketSet => fpp_lsp_parser::TopEntryPoint::TlmPacketSet,
        fpp_parser::IncludeParentKind::Topology => fpp_lsp_parser::TopEntryPoint::Topology,
    };

    eprintln!(
        "CST {}: entry {entry_kind:?}, source_file: {source_file:?}",
        param.uri.as_str()
    );
    eprintln!("{}", parse.debug_dump());

    Ok(())
}

fn parse_text_document(
    state: &GlobalState,
    uri: &Uri,
) -> Result<(String, Option<SourceFile>, fpp_lsp_parser::Parse)> {
    let text: String = state.vfs.read_sync(uri.as_str())?;

    let source_file = state
        .files
        .get(uri.as_str())
        .and_then(|files| files.first().cloned());

    let parse_kind = source_file
        .and_then(|f| state.analysis.include_context_map.get(&f).cloned())
        .unwrap_or(fpp_parser::IncludeParentKind::Module);

    let entry_kind = match parse_kind {
        fpp_parser::IncludeParentKind::Component => fpp_lsp_parser::TopEntryPoint::Component,
        fpp_parser::IncludeParentKind::Module => fpp_lsp_parser::TopEntryPoint::Module,
        fpp_parser::IncludeParentKind::TlmPacket => fpp_lsp_parser::TopEntryPoint::TlmPacket,
        fpp_parser::IncludeParentKind::TlmPacketSet => fpp_lsp_parser::TopEntryPoint::TlmPacketSet,
        fpp_parser::IncludeParentKind::Topology => fpp_lsp_parser::TopEntryPoint::Topology,
    };

    let parse = fpp_lsp_parser::parse(&text, entry_kind);
    Ok((text, source_file, parse))
}

pub fn handle_semantic_tokens_full(
    state: &mut GlobalState,
    request: lsp_types::SemanticTokensParams,
) -> Result<Option<SemanticTokensResult>> {
    let start = std::time::Instant::now();
    let (text, src, parse) = parse_text_document(state, &request.text_document.uri)?;
    let parsed = start.elapsed();
    let semantic_tokens = lsp::semantic_tokens::compute(state, src, &text, &parse).finish(None);
    tracing::info!(
        uri = %request.text_document.uri.as_str(),
        text_len = text.len(),
        token_count = semantic_tokens.data.len(),
        parse = format_args!("{:0.2?}", parsed),
        total = format_args!("{:0.2?}", start.elapsed()),
        "semantic tokens full"
    );

    // Unconditionally cache the tokens
    state
        .semantic_tokens
        .insert(request.text_document.uri, semantic_tokens.clone());

    Ok(Some(semantic_tokens.into()))
}

pub fn handle_semantic_tokens_range(
    state: &GlobalState,
    request: lsp_types::SemanticTokensRangeParams,
) -> Result<Option<SemanticTokensRangeResult>> {
    let (text, src, parse) = parse_text_document(state, &request.text_document.uri)?;

    Ok(Some(SemanticTokensRangeResult::Tokens(
        lsp::semantic_tokens::compute(state, src, &text, &parse).finish(Some(request.range)),
    )))
}

pub fn handle_semantic_tokens_full_delta(
    state: &mut GlobalState,
    request: lsp_types::SemanticTokensDeltaParams,
) -> Result<Option<SemanticTokensFullDeltaResult>> {
    let start = std::time::Instant::now();
    let (text, src, parse) = parse_text_document(state, &request.text_document.uri)?;

    let semantic_tokens = lsp::semantic_tokens::compute(state, src, &text, &parse).finish(None);

    let cached_tokens = state.semantic_tokens.remove(&request.text_document.uri);

    if let Some(
        cached_tokens @ lsp_types::SemanticTokens {
            result_id: Some(prev_id),
            ..
        },
    ) = &cached_tokens
        && *prev_id == request.previous_result_id
    {
        let delta = semantic_token_delta(cached_tokens, &semantic_tokens);
        tracing::info!(
            uri = %request.text_document.uri.as_str(),
            text_len = text.len(),
            token_count = semantic_tokens.data.len(),
            edits = delta.edits.len(),
            total = format_args!("{:0.2?}", start.elapsed()),
            "semantic tokens delta"
        );
        state
            .semantic_tokens
            .insert(request.text_document.uri, semantic_tokens);
        return Ok(Some(delta.into()));
    }

    // Clone first to keep the lock short
    let semantic_tokens_clone = semantic_tokens.clone();
    state
        .semantic_tokens
        .insert(request.text_document.uri, semantic_tokens_clone);

    Ok(Some(semantic_tokens.into()))
}

pub fn handle_document_diagnostics(
    state: &mut GlobalState,
    request: lsp_types::DocumentDiagnosticParams,
) -> Result<DocumentDiagnosticReportResult> {
    Ok(DocumentDiagnosticReportResult::Report(
        lsp_types::DocumentDiagnosticReport::Full(lsp_types::RelatedFullDocumentDiagnosticReport {
            full_document_diagnostic_report: lsp_types::FullDocumentDiagnosticReport {
                items: state.diagnostics.get(request.text_document.uri.as_str()),
                ..Default::default()
            },
            ..Default::default()
        }),
    ))
}

fn text_range_to_range(lines: &LineIndex, text_range: TextRange) -> Range {
    let start = lines.line_col(text_range.start());
    let end = lines.line_col(text_range.end());

    Range {
        start: Position {
            line: start.line,
            character: start.col,
        },
        end: Position {
            line: end.line,
            character: end.col,
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DocumentLinkData {
    pub origin_uri: Uri,
    pub relative_path: String,
}

struct DocumentLinksVisitor<'a> {
    uri: Uri,
    lines: &'a LineIndex,
    text: &'a str,
}
impl<'a> fpp_lsp_parser::Visitor for DocumentLinksVisitor<'a> {
    type State = Vec<DocumentLink>;

    fn visit_node(&self, state: &mut Self::State, node: &SyntaxNode) -> VisitorResult {
        match node.kind() {
            SyntaxKind::ROOT => VisitorResult::Recurse,
            SyntaxKind::DEF_MODULE => VisitorResult::Recurse,
            SyntaxKind::MODULE_MEMBER_LIST => VisitorResult::Recurse,
            SyntaxKind::DEF_COMPONENT => VisitorResult::Recurse,
            SyntaxKind::COMPONENT_MEMBER_LIST => VisitorResult::Recurse,
            SyntaxKind::DEF_STATE_MACHINE => VisitorResult::Recurse,
            SyntaxKind::STATE_MACHINE_MEMBER_LIST => VisitorResult::Recurse,
            SyntaxKind::DEF_STATE => VisitorResult::Recurse,
            SyntaxKind::STATE_MEMBER_LIST => VisitorResult::Recurse,
            SyntaxKind::DEF_TOPOLOGY => VisitorResult::Recurse,
            SyntaxKind::TOPOLOGY_MEMBER_LIST => VisitorResult::Recurse,
            SyntaxKind::TLM_PACKET_SET => VisitorResult::Recurse,
            SyntaxKind::TLM_PACKET_SET_MEMBER_LIST => VisitorResult::Recurse,
            SyntaxKind::SPEC_TLM_PACKET => VisitorResult::Recurse,
            SyntaxKind::TLM_PACKET_MEMBER_LIST => VisitorResult::Recurse,

            SyntaxKind::SPEC_INCLUDE => {
                // Get the string literal noting the file to include
                match node.first_child_or_token_by_kind(&|t| t == SyntaxKind::LITERAL_STRING) {
                    None => {}
                    Some(file) => {
                        // This token is either "file.fppi" or """file.fppi"""
                        // We need to strip off the quotes
                        let file_include = {
                            let file_include_text = &self.text[file.text_range()];
                            if let Some(inner) = file_include_text.strip_prefix("\"\"\"") {
                                // Triple-quoted string; strip the matching suffix if present,
                                // otherwise fall back to the off-nominal remainder.
                                inner.strip_suffix("\"\"\"").unwrap_or(inner)
                            } else {
                                &file_include_text[1..file_include_text.len() - 1]
                            }
                        };

                        let link = DocumentLink {
                            range: text_range_to_range(self.lines, file.text_range()),
                            target: None,
                            tooltip: None,
                            data: Some(
                                serde_json::to_value(DocumentLinkData {
                                    origin_uri: self.uri.clone(),
                                    relative_path: file_include.to_string(),
                                })
                                .unwrap(),
                            ),
                        };

                        state.push(link);
                    }
                }

                VisitorResult::Next
            }

            _ => VisitorResult::Next,
        }
    }

    fn visit_token(&self, _: &mut Self::State, _: &SyntaxToken) {}
}

pub fn handle_document_link_request(
    state: &GlobalState,
    request: lsp_types::DocumentLinkParams,
) -> Result<Option<Vec<DocumentLink>>> {
    let (text, _, parse) = parse_text_document(state, &request.text_document.uri)?;
    let lines = state.vfs.get_lines(request.text_document.uri.as_str())?;

    let mut links = vec![];
    parse.visit(
        &mut links,
        &DocumentLinksVisitor {
            uri: request.text_document.uri.clone(),
            lines: &lines,
            text: &text,
        },
    );
    if links.is_empty() {
        Ok(None)
    } else {
        Ok(Some(links))
    }
}

pub fn handle_document_link_resolve(
    state: &GlobalState,
    request: DocumentLink,
) -> Result<DocumentLink> {
    let data: DocumentLinkData = match request.data {
        None => return Err(anyhow::anyhow!("Document link has no data to resolve")),
        Some(data) => serde_json::from_value(data)?,
    };

    let resolved = state
        .vfs
        .resolve_uri_relative_path(data.origin_uri.as_str(), &data.relative_path)?;
    Ok(DocumentLink {
        range: request.range,
        target: Some(Uri::from_str(&resolved)?),
        tooltip: None,
        data: None,
    })
}

pub fn handle_goto_definition(
    state: &GlobalState,
    request: GotoDefinitionParams,
) -> Result<Option<GotoDefinitionResponse>> {
    let offset = position_to_offset(
        state,
        &request.text_document_position_params.text_document.uri,
        &request.text_document_position_params.position,
    );

    let uri = &request.text_document_position_params.text_document.uri;

    // Port names in connections resolve to a port instance rather than a symbol;
    // jump to the port instance's declaration. This must be checked *before*
    // `symbol_at_position`: for a qualified port (`A.Sub.someOut`) the instance
    // qualifier's use node spans through the port name, so the generic symbol
    // lookup would otherwise resolve the port name to the (sub)topology.
    // `port_instance_at_position` only matches when the cursor is on the port
    // name itself, so it does not interfere with the qualifier.
    if let Some((_, port_instance)) = port_instance_at_position(state, uri, offset) {
        Ok(Some(GotoDefinitionResponse::Scalar(node_to_location(
            state,
            port_instance.get_node_id(),
        ))))
    } else if let Some((_, port_instance)) = port_match_at_position(state, uri, offset) {
        // Port names in a match specifier (`match a with b`) resolve to a port
        // instance rather than a symbol; jump to its declaration.
        Ok(Some(GotoDefinitionResponse::Scalar(node_to_location(
            state,
            port_instance.get_node_id(),
        ))))
    } else if let Some((_, _, sm_symbol)) = sm_symbol_at_position(state, uri, offset) {
        // Uses inside a state machine resolve via the state machine's own
        // use-def map, not the global one; jump to the definition.
        Ok(Some(GotoDefinitionResponse::Scalar(node_to_location(
            state,
            sm_symbol.name().id(),
        ))))
    } else if let Some((_, symbol)) = symbol_at_position(state, uri, offset) {
        Ok(Some(GotoDefinitionResponse::Scalar(node_to_location(
            state,
            symbol.name().id(),
        ))))
    } else {
        Ok(None)
    }
}

pub fn handle_hover(state: &GlobalState, request: HoverParams) -> Result<Option<Hover>> {
    let offset = position_to_offset(
        state,
        &request.text_document_position_params.text_document.uri,
        &request.text_document_position_params.position,
    );

    let nodes = match nodes_at_offset(
        state,
        &request.text_document_position_params.text_document.uri,
        offset,
    ) {
        None => return Ok(None),
        Some(nodes) => nodes,
    };

    // Port names in connections resolve to a port instance, not a symbol, so
    // they are absent from use_def_map. Resolve them first: for a qualified port
    // (`A.Sub.someOut`) the instance qualifier's use node spans through the port
    // name, so the generic use_def_map lookup below would otherwise show the
    // (sub)topology. `port_instance_at_position` only matches when the cursor is
    // on the port name, so it does not shadow the qualifier's own hover.
    if let Some((name_node, port_instance)) = port_instance_at_position(
        state,
        &request.text_document_position_params.text_document.uri,
        offset,
    ) {
        return Ok(Some(hover_for_port_instance(
            state,
            name_node,
            &port_instance,
        )));
    }

    // Port names in a match specifier (`match a with b`) are bare identifiers,
    // not symbols, so they are absent from use_def_map. Resolve them against the
    // enclosing component's port map.
    if let Some((name_node, port_instance)) = port_match_at_position(
        state,
        &request.text_document_position_params.text_document.uri,
        offset,
    ) {
        return Ok(Some(hover_for_port_instance(
            state,
            name_node,
            &port_instance,
        )));
    }

    // Uses inside a state machine resolve via the state machine's own use-def
    // map, not the global one; resolve them before the generic lookup.
    if let Some((node, state_machine, sm_symbol)) = sm_symbol_at_position(
        state,
        &request.text_document_position_params.text_document.uri,
        offset,
    ) {
        return Ok(Some(hover_for_sm_symbol(
            state,
            node,
            state_machine,
            &sm_symbol,
        )));
    }

    // State machine member *definitions* (actions, guards, signals, states,
    // choices) are absent from the global symbol_map — they live only in their
    // state machine's analysis. Resolve a hover on such a definition name here
    // so it shows the member's own info; otherwise it would fall through to the
    // enclosing state machine below and show the parent state machine instead.
    if let Some((node, state_machine, sm_symbol)) = sm_def_at_position(
        state,
        &request.text_document_position_params.text_document.uri,
        offset,
    ) {
        return Ok(Some(hover_for_sm_symbol(
            state,
            node,
            state_machine,
            &sm_symbol,
        )));
    }

    // Check if this node is a use/reference to definition
    if let Some((node, symbol)) = nodes.iter().find_map(|node| {
        state
            .analysis
            .use_def_map
            .get(&node.id())
            .map(|def| (*node, def))
    }) {
        return Ok(Some(hover_for_symbol(state, node, symbol)));
    }

    // This is not a use/reference to another definition
    // From here on in we should only show hover information for definitions if we are hovering over
    // the definition's name
    if let Some(Node::Name(name)) = nodes.first() {
        // We are hovering over a name
        Ok(nodes
            .iter()
            .find_map(|node| hover_for_node(state, name, *node)))
    } else {
        Ok(None)
    }
}

pub fn handle_references(
    state: &GlobalState,
    request: ReferenceParams,
) -> Result<Option<Vec<Location>>> {
    let offset = position_to_offset(
        state,
        &request.text_document_position.text_document.uri,
        &request.text_document_position.position,
    );

    if let Some(nodes) = nodes_at_offset(
        state,
        &request.text_document_position.text_document.uri,
        offset,
    ) {
        let symbol = {
            // Check if this is a use to a symbol
            if let Some(symbol) = nodes
                .iter()
                .find_map(|node| state.analysis.use_def_map.get(&node.id()))
            {
                Some(symbol)
            // Check if this is a symbol definition
            } else if let Some(symbol) = nodes
                .iter()
                .find_map(|node| state.analysis.symbol_map.get(&node.id()))
                && let Some(Node::Name(_)) = nodes.first()
            {
                Some(symbol)
            } else {
                None
            }
        };

        if let Some(symbol) = symbol {
            // Look for all use-def resolutions that map to this symbol
            Ok(Some(
                state
                    .analysis
                    .use_def_map
                    .iter()
                    .filter_map(|(node, i_symbol)| {
                        if symbol.node() == i_symbol.node() {
                            Some(node_to_location(state, *node))
                        } else {
                            None
                        }
                    })
                    .collect(),
            ))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

/// Determine the required port direction for a port-instance identifier, based
/// on which connection endpoint (if any) encloses it.
///
/// The `from` side of a connection accepts only output ports and the `to` side
/// only input ports. A topology port alias (`port Foo = i.p`) may re-export a
/// port of either direction, so it is left unfiltered (`None`) — the direction
/// of the port being *used* is still surfaced via each item's detail.
fn connection_endpoint_direction(pii: &SyntaxNode) -> Option<fpp_analysis::semantics::Direction> {
    use fpp_analysis::semantics::Direction;
    pii.ancestors().find_map(|s| match s.kind() {
        SyntaxKind::CONNECTION_FROM => Some(Direction::Output),
        SyntaxKind::CONNECTION_TO => Some(Direction::Input),
        _ => None,
    })
}

/// Classify a state machine completion context from the token to the left of
/// the cursor.
///
/// State machine transitions reference four kinds of definitions in distinct
/// syntactic positions:
///   * actions in `do { <here> }`
///   * a guard after `if <here>`
///   * a signal after `on <here>`
///   * a state or choice after `enter <here>`
///
/// The context is recognized either from the keyword just typed (`do`/`if`/
/// `on`/`enter`, when nothing follows yet) or from the enclosing grammar rule
/// (when a partial name is being typed). `enter` targets are checked before the
/// dot-qualified `QUAL_IDENT` handling elsewhere, since that path returns early
/// for `TRANSITION_EXPR`.
fn state_machine_completion_kind(
    left_token: &SyntaxToken,
    cursor_pos: fpp_lsp_parser::TextSize,
) -> Option<crate::util::SmCompletionKind> {
    use crate::util::SmCompletionKind;

    // A `do { ... }` action list: the cursor is within the do-expression, and
    // any preceding `enter` (which would make this a transition target) is not
    // an ancestor of a do-expression member.
    let in_do_expr = left_token
        .parent_ancestors()
        .take_while(|s| s.kind() != SyntaxKind::TRANSITION_EXPR)
        .any(|s| s.kind() == SyntaxKind::DO_EXPR);
    if in_do_expr {
        return Some(SmCompletionKind::Action);
    }

    // Directly after one of the introducing keywords, before a name is typed.
    match left_token.kind() {
        SyntaxKind::DO_KW => return Some(SmCompletionKind::Action),
        SyntaxKind::IF_KW => return Some(SmCompletionKind::Guard),
        SyntaxKind::ON_KW => return Some(SmCompletionKind::Signal),
        SyntaxKind::ENTER_KW => return Some(SmCompletionKind::StateOrChoice),
        _ => {}
    }

    // A state/choice transition target: `enter <qual ident>`.
    if left_token
        .parent_ancestors()
        .any(|s| s.kind() == SyntaxKind::TRANSITION_EXPR)
    {
        return Some(SmCompletionKind::StateOrChoice);
    }

    // Within a state transition (`on <signal> [if <guard>] ...`): a `NAME_REF`
    // is the guard; the bare signal identifier sits directly under the spec.
    if let Some(spec) = left_token
        .parent_ancestors()
        .find(|s| s.kind() == SyntaxKind::SPEC_STATE_TRANSITION)
    {
        if left_token
            .parent_ancestors()
            .any(|s| s.kind() == SyntaxKind::NAME_REF)
        {
            return Some(SmCompletionKind::Guard);
        }
        // The signal is the first identifier of the spec; only treat the cursor
        // as the signal position while it is at or before that identifier.
        if let Some(signal_ident) = spec
            .children_with_tokens()
            .filter_map(|c| c.into_token())
            .find(|t| t.kind() == SyntaxKind::IDENT)
            && cursor_pos <= signal_ident.text_range().end()
        {
            return Some(SmCompletionKind::Signal);
        }
    }

    // The guard of a choice definition: `choice C { if <here> ... }`.
    if left_token
        .parent_ancestors()
        .any(|s| s.kind() == SyntaxKind::NAME_REF)
        && left_token
            .parent_ancestors()
            .any(|s| s.kind() == SyntaxKind::DEF_CHOICE)
    {
        return Some(SmCompletionKind::Guard);
    }

    None
}

pub fn handle_completion(
    state: &GlobalState,
    request: CompletionParams,
) -> Result<Option<CompletionResponse>> {
    let uri = request.text_document_position.text_document.uri;

    let text: String = state.vfs.read_sync(uri.as_str())?;
    let lines = state.vfs.get_lines(uri.as_str())?;

    let cursor_pos = match lines.offset(LineCol {
        line: request.text_document_position.position.line,
        col: request.text_document_position.position.character,
    }) {
        None => return Err(anyhow::anyhow!("position not in file bounds")),
        Some(p) => p,
    };

    let parse_kind = match state.files.get(uri.as_str()) {
        None => fpp_parser::IncludeParentKind::Module,
        Some(source_files) => {
            // This file may have been included in multiple spots
            // We should choose the most 'permissive' syntax entry point
            source_files
                .iter()
                .map(|f| match state.analysis.include_context_map.get(f) {
                    Some(kind) => *kind,
                    None => fpp_parser::IncludeParentKind::Module,
                })
                .max()
                .unwrap_or(fpp_parser::IncludeParentKind::Module)
        }
    };

    let entry_kind = match parse_kind {
        fpp_parser::IncludeParentKind::Component => fpp_lsp_parser::TopEntryPoint::Component,
        fpp_parser::IncludeParentKind::Module => fpp_lsp_parser::TopEntryPoint::Module,
        fpp_parser::IncludeParentKind::TlmPacket => fpp_lsp_parser::TopEntryPoint::TlmPacket,
        fpp_parser::IncludeParentKind::TlmPacketSet => fpp_lsp_parser::TopEntryPoint::TlmPacketSet,
        fpp_parser::IncludeParentKind::Topology => fpp_lsp_parser::TopEntryPoint::Topology,
    };

    let parse = fpp_lsp_parser::parse(&text, entry_kind);

    fn non_white_space_left(root: &SyntaxNode, mut l: SyntaxToken) -> SyntaxToken {
        loop {
            match l.kind() {
                SyntaxKind::EOL
                | SyntaxKind::COMMENT
                | SyntaxKind::WHITESPACE
                | SyntaxKind::PRE_ANNOTATION
                | SyntaxKind::POST_ANNOTATION => {
                    // `prev_token()` dead-ends on trailing zero-width nodes (e.g.
                    // an empty `CONNECTION_TO` after `->`). Fall back to
                    // `token_at_offset` at this token's start, which crosses
                    // empty nodes; the range guard prevents looping on self.
                    let prev = l.prev_token().or_else(|| {
                        match root.token_at_offset(l.text_range().start()) {
                            TokenAtOffset::Between(a, _) => Some(a),
                            TokenAtOffset::Single(t) => Some(t),
                            TokenAtOffset::None => None,
                        }
                        .filter(|t| t.text_range() != l.text_range())
                    });
                    l = match prev {
                        Some(ll) => ll,
                        None => return l,
                    };
                }
                _ => return l,
            }
        }
    }

    fn non_white_space_right(mut r: SyntaxToken) -> SyntaxToken {
        loop {
            match r.kind() {
                SyntaxKind::EOL
                | SyntaxKind::COMMENT
                | SyntaxKind::WHITESPACE
                | SyntaxKind::PRE_ANNOTATION
                | SyntaxKind::POST_ANNOTATION => {
                    r = match r.next_token() {
                        Some(ll) => ll,
                        None => return r,
                    };
                }
                _ => return r,
            }
        }
    }

    let root = parse.syntax_node();
    let cursor_token = root.token_at_offset(cursor_pos);

    // Don't offer completions when the cursor sits inside a comment. Comments are
    // trivia the generic branches below happily skip over (via
    // `non_white_space_left`), so without this guard we'd complete against the
    // preceding code token as if the comment weren't there. A comment spans
    // `[start, end)`; treat the cursor as "inside" it anywhere past the leading
    // `#` (`Single`), and at the trailing edge (`Between` with the comment on the
    // left). The leading boundary itself (`Between` with the comment on the
    // right) is still real code position, so completion is allowed there.
    let in_comment = match &cursor_token {
        TokenAtOffset::None => false,
        TokenAtOffset::Single(tok) => tok.kind() == SyntaxKind::COMMENT,
        TokenAtOffset::Between(l, _) => l.kind() == SyntaxKind::COMMENT,
    };
    if in_comment {
        return Ok(None);
    }

    let expected_error_range = match &cursor_token {
        TokenAtOffset::None => return Ok(None),
        TokenAtOffset::Single(tok) => non_white_space_left(&root, tok.clone())
            .text_range()
            .cover(non_white_space_right(tok.clone()).text_range()),
        TokenAtOffset::Between(l, r) => non_white_space_left(&root, l.clone())
            .text_range()
            .cover(non_white_space_right(r.clone()).text_range()),
    };

    let left_token = match &cursor_token {
        TokenAtOffset::None => unreachable!(),
        TokenAtOffset::Single(tok) => non_white_space_left(&root, tok.clone()),
        TokenAtOffset::Between(l, _) => non_white_space_left(&root, l.clone()),
    };

    // State/choice transition targets (`enter A.B.C`) descend one nesting level
    // at a time. This is handled before the generic qualified-identifier branch
    // and independently of whether the cursor follows a dot, so it covers the
    // first segment (`enter A`), a partial child (`enter A.B`), and just after a
    // dot (`enter A.`). The qualifier is every identifier before the final dot;
    // the (possibly partial) segment at the cursor is excluded.
    if let Some(qual_ident) = left_token
        .parent_ancestors()
        .find(|s| s.kind() == SyntaxKind::QUAL_IDENT)
        && qual_ident.parent().map(|p| p.kind()) == Some(SyntaxKind::TRANSITION_EXPR)
    {
        let last_dot_end = qual_ident
            .descendants_with_tokens()
            .filter_map(|s| s.as_token().cloned())
            .filter(|t| t.kind() == SyntaxKind::DOT && t.text_range().end() <= cursor_pos)
            .map(|t| t.text_range().end())
            .max();

        let qualifier: Vec<String> = qual_ident
            .descendants_with_tokens()
            .filter_map(|s| s.as_token().cloned())
            .filter(|t| {
                t.kind() == SyntaxKind::IDENT
                    && last_dot_end.is_some_and(|dot_end| t.text_range().end() < dot_end)
            })
            .map(|t| t.text().to_string())
            .collect();

        return Ok(
            completion_items_for_sm_state(state, &uri, cursor_pos.into(), &qualifier)
                .map(CompletionResponse::Array),
        );
    }

    if left_token.kind() == SyntaxKind::DOT
        && let Some(qual_ident) = left_token
            .parent_ancestors()
            .find(|s| s.kind() == SyntaxKind::QUAL_IDENT)
    {
        // Complete member selection on qualified identifiers

        let parent_rule = match qual_ident.parent() {
            None => return Ok(None),
            Some(r) => r,
        };

        let ng = match parent_rule.kind() {
            SyntaxKind::DEF_COMPONENT_INSTANCE => NameGroup::Component,
            SyntaxKind::IMPLEMENTS_CLAUSE => NameGroup::PortInterface,
            SyntaxKind::SPEC_CONNECTION_GRAPH_PATTERN => NameGroup::PortInterfaceInstance,
            SyntaxKind::PATTERN_TARGET_MEMBER_LIST => NameGroup::PortInterfaceInstance,
            SyntaxKind::SPEC_INTERFACE_IMPORT => NameGroup::PortInterface,
            SyntaxKind::SPEC_INSTANCE => NameGroup::PortInterfaceInstance,
            SyntaxKind::SPEC_LOC => return Ok(None),
            SyntaxKind::SPEC_PORT_INSTANCE_GENERAL => NameGroup::Port,
            SyntaxKind::SPEC_STATE_MACHINE_INSTANCE => NameGroup::StateMachine,
            SyntaxKind::TYPE_NAME => NameGroup::Type,
            _ => return Ok(None),
        };

        let tokens: Vec<SyntaxToken> = qual_ident
            .descendants_with_tokens()
            .filter_map(|s| s.as_token().cloned())
            .filter(|t| t.kind() == SyntaxKind::IDENT && t.text_range().end() <= cursor_pos)
            .collect();

        // Get the final token before the cursor to look it up in the AST
        let last_token_pos = tokens.last().unwrap().text_range().start().into();

        // Look up the symbol before the cursor
        Ok(symbol_at_position(state, &uri, last_token_pos)
            .and_then(|(_, symbol)| state.analysis.symbol_scope_map.get(&symbol))
            .map(|scope| {
                // Get all symbols under this symbol's scope in the proper
                // name group
                scope
                    .get_group(ng)
                    .iter()
                    .map(|(_, child_symbol)| symbol_to_completion_item(state, child_symbol))
            })
            .map(|s| CompletionResponse::Array(s.collect())))
    } else if left_token.kind() == SyntaxKind::DOT
        && let Some(postfix_expr) = left_token
            .parent_ancestors()
            .find(|s| s.kind() == SyntaxKind::EXPR_POSTFIX)
    {
        // Member selection on value expressions (e.g. `a.b.c.`)
        Ok(
            completion_items_for_postfix_expr(state, &postfix_expr, cursor_pos, &uri)
                .map(CompletionResponse::Array),
        )
    } else if let Some(pii) = left_token
        .parent_ancestors()
        .find(|s| s.kind() == SyntaxKind::PORT_INSTANCE_IDENTIFIER)
    {
        // Completion inside a port-instance identifier (`instance.PortName`),
        // used by topology connection endpoints and topology port aliases.
        //
        // The identifier is a flat `IDENT (DOT IDENT)*` sequence. If the cursor
        // follows a dot we are completing a port name (or a nested instance);
        // otherwise we are on the first bare segment and complete instance names.
        let last_dot_start = pii
            .descendants_with_tokens()
            .filter_map(|s| s.as_token().cloned())
            .filter(|t| t.kind() == SyntaxKind::DOT && t.text_range().end() <= cursor_pos)
            .map(|t| t.text_range().start())
            .max();

        let after_dot = last_dot_start.is_some();

        // The receiver is every identifier before the final relevant dot.
        let receiver_tokens: Vec<SyntaxToken> = match last_dot_start {
            Some(dot_start) => pii
                .descendants_with_tokens()
                .filter_map(|s| s.as_token().cloned())
                .filter(|t| t.kind() == SyntaxKind::IDENT && t.text_range().end() <= dot_start)
                .collect(),
            None => vec![],
        };

        // Ports on the `from` side of `->` must be outputs, the `to` side
        // inputs. Topology port aliases (and anything else) are unfiltered.
        let direction_filter = connection_endpoint_direction(&pii);

        Ok(completion_items_for_port_instance(
            state,
            &receiver_tokens,
            after_dot,
            direction_filter,
            cursor_pos,
            &uri,
        )
        .map(CompletionResponse::Array))
    } else if left_token.kind() == SyntaxKind::RIGHT_ARROW
        && left_token
            .parent_ancestors()
            .any(|s| s.kind() == SyntaxKind::CONNECTION)
    {
        // Just after the `->` in a connection (`a.b -> `), before any `to`
        // endpoint identifier has been typed. The empty CONNECTION_TO endpoint
        // does not contain the cursor, so complete the first segment (an
        // interface-instance name) directly.
        Ok(completion_items_for_port_instance(
            state,
            &[],
            false,
            Some(fpp_analysis::semantics::Direction::Input),
            cursor_pos,
            &uri,
        )
        .map(CompletionResponse::Array))
    } else if let Some(match_node) = left_token
        .parent_ancestors()
        .find(|s| s.kind() == SyntaxKind::MATCH_KW)
    {
        // Completion for a port-match specifier (`match a with b`). The node
        // (kind `MATCH_KW`) holds two `NAME_REF` port names. Per the matched
        // numbering convention the first is an output port and the second an
        // input port. Decide which name the cursor is on: everything up to and
        // including the `with` keyword is the first (output) name; after it is
        // the second (input) name.
        // Use `descendants_with_tokens` since an incomplete first name parks the
        // `with` keyword inside an ERROR node rather than as a direct child.
        let with_end = match_node
            .descendants_with_tokens()
            .filter_map(|c| c.into_token())
            .find(|t| t.kind() == SyntaxKind::WITH_KW)
            .map(|t| t.text_range().end());

        let direction = match with_end {
            Some(end) if cursor_pos >= end => fpp_analysis::semantics::Direction::Input,
            _ => fpp_analysis::semantics::Direction::Output,
        };

        Ok(
            completion_items_for_port_match(state, &uri, cursor_pos.into(), direction)
                .map(CompletionResponse::Array),
        )
    } else if let Some(sm_kind) = state_machine_completion_kind(&left_token, cursor_pos) {
        // Completion for state machine definitions: actions in `do { }`, guards
        // after `if`, signals after `on`, and states/choices after `enter`.
        Ok(
            completion_items_for_sm(state, &uri, cursor_pos.into(), sm_kind)
                .map(CompletionResponse::Array),
        )
    } else {
        // Check for parsing errors to extract the next expected token
        Ok(Some(CompletionResponse::Array(
            parse
                .errors()
                .iter()
                .filter_map(|e| {
                    if expected_error_range.intersect(e.range()).is_some()
                        && let Some(expected_kind) = e.expected()
                    {
                        match expected_kind {
                            keyword if keyword.is_keyword() => {
                                // FIXME(tumbar) This seems brittle but works ok for now
                                let keyword_dbg = format!("{:?}", keyword);
                                assert!(keyword_dbg.ends_with("_KW"));
                                let keyword_s =
                                    keyword_dbg[..keyword_dbg.len() - 3].to_ascii_lowercase();

                                Some(vec![CompletionItem {
                                    label: keyword_s,
                                    kind: Some(CompletionItemKind::KEYWORD),
                                    ..Default::default()
                                }])
                            }
                            SyntaxKind::QUAL_IDENT => {
                                let element = parse
                                    .syntax_node()
                                    .covering_element(TextRange::new(cursor_pos, cursor_pos));

                                if let Some(ng) = element.ancestors().find_map(|n| match n.kind() {
                                    SyntaxKind::DEF_COMPONENT_INSTANCE => {
                                        Some(NameGroup::Component)
                                    }
                                    SyntaxKind::IMPLEMENTS_CLAUSE => Some(NameGroup::PortInterface),
                                    SyntaxKind::SPEC_CONNECTION_GRAPH_PATTERN => {
                                        Some(NameGroup::PortInterfaceInstance)
                                    }
                                    SyntaxKind::PATTERN_TARGET_MEMBER_LIST => {
                                        Some(NameGroup::PortInterfaceInstance)
                                    }
                                    SyntaxKind::SPEC_INTERFACE_IMPORT => {
                                        Some(NameGroup::PortInterface)
                                    }
                                    SyntaxKind::SPEC_INSTANCE => {
                                        Some(NameGroup::PortInterfaceInstance)
                                    }
                                    SyntaxKind::SPEC_PORT_INSTANCE_GENERAL => Some(NameGroup::Port),
                                    SyntaxKind::SPEC_STATE_MACHINE_INSTANCE => {
                                        Some(NameGroup::StateMachine)
                                    }
                                    SyntaxKind::TYPE_NAME => Some(NameGroup::Type),
                                    _ => None,
                                }) {
                                    completion_items_in_name_group(state, cursor_pos, ng, &uri)
                                } else {
                                    None
                                }
                            }
                            SyntaxKind::TYPE_NAME => {
                                let element = parse
                                    .syntax_node()
                                    .covering_element(TextRange::new(cursor_pos, cursor_pos));

                                let symbol_completions = completion_items_for_qual_ident(
                                    state,
                                    element,
                                    cursor_pos,
                                    NameGroup::Type,
                                    &uri,
                                )
                                .unwrap_or_default();

                                let keyword_completions: Vec<CompletionItem> = vec![
                                    "U8", "I8", "U16", "I16", "U32", "I32", "U64", "I64", "string",
                                    "bool",
                                ]
                                .into_iter()
                                .map(|kw| CompletionItem {
                                    label: kw.to_string(),
                                    kind: Some(CompletionItemKind::CLASS),
                                    ..Default::default()
                                })
                                .collect();

                                Some(
                                    keyword_completions
                                        .into_iter()
                                        .chain(symbol_completions)
                                        .collect(),
                                )
                            }
                            SyntaxKind::EXPR => {
                                let element = parse
                                    .syntax_node()
                                    .covering_element(TextRange::new(cursor_pos, cursor_pos));

                                completion_items_for_qual_ident(
                                    state,
                                    element,
                                    cursor_pos,
                                    NameGroup::Value,
                                    &uri,
                                )
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
                .flatten()
                .collect(),
        )))
    }
}

/// Resolve the [`fpp_format::FormatOptions`] for a document by discovering the
/// nearest `.fpp-format` file, walking up from the document's own directory.
///
/// The language server never applies command-line overrides, so the profile is
/// the discovered file layered over the built-in defaults. A missing config
/// yields defaults; a malformed config is logged and defaults are used (a
/// format-on-save must not fail the editor over a bad config file).
fn resolve_format_options(uri: &lsp_types::Uri) -> fpp_format::FormatOptions {
    let path = std::path::Path::new(uri.path().as_str());
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    match fpp_format::load_config(dir) {
        Ok(config) => config.into_options(),
        Err(err) => {
            tracing::warn!(
                "ignoring malformed {}: {}",
                fpp_format::CONFIG_FILE_NAME,
                err
            );
            fpp_format::FormatOptions::default()
        }
    }
}

pub fn handle_formatting(
    state: &GlobalState,
    request: DocumentFormattingParams,
) -> Result<Option<Vec<TextEdit>>> {
    let lines = state.vfs.get_lines(request.text_document.uri.as_str())?;
    let (original, _, parse) = parse_text_document(state, &request.text_document.uri)?;

    if !parse.errors().is_empty() {
        tracing::warn!("Cannot format with parse errors: {:?}", parse.errors());
        return Ok(None);
    }

    // Format the entire document
    let syntax = parse.syntax_node();
    let options = resolve_format_options(&request.text_document.uri);
    let text = fpp_format::Formatter::new(options).format(&syntax);

    // Replace the range spanning the entire *original* document. The range must
    // be derived from the original length, not the formatted length, otherwise
    // a shorter result leaves trailing original bytes (e.g. a stray `}`) behind.
    let orig_len = fpp_lsp_parser::TextSize::from(original.len() as u32);
    let text_range = fpp_lsp_parser::TextRange::new(0.into(), orig_len);
    let range = text_range_to_range(&lines, text_range);

    Ok(Some(vec![TextEdit {
        range,
        new_text: text,
    }]))
}

pub fn handle_range_formatting(
    state: &GlobalState,
    request: DocumentRangeFormattingParams,
) -> Result<Option<Vec<TextEdit>>> {
    // For initial implementation, format the entire document
    // Future optimization: extract and format only the specified range
    let lines = state.vfs.get_lines(request.text_document.uri.as_str())?;
    let (original, _, parse) = parse_text_document(state, &request.text_document.uri)?;

    if !parse.errors().is_empty() {
        tracing::warn!("Cannot format with parse errors: {:?}", parse.errors());
        return Ok(None);
    }

    // Format the entire document
    let syntax = parse.syntax_node();
    let options = resolve_format_options(&request.text_document.uri);
    let text = fpp_format::Formatter::new(options).format(&syntax);

    // Replace the range spanning the entire *original* document. The range must
    // be derived from the original length, not the formatted length, otherwise
    // a shorter result leaves trailing original bytes (e.g. a stray `}`) behind.
    let orig_len = fpp_lsp_parser::TextSize::from(original.len() as u32);
    let text_range = fpp_lsp_parser::TextRange::new(0.into(), orig_len);
    let range = text_range_to_range(&lines, text_range);

    Ok(Some(vec![TextEdit {
        range,
        new_text: text,
    }]))
}

#[cfg(test)]
mod hover_tests {
    use super::*;
    use crate::global_state::GlobalState;
    use lsp_types::{
        HoverContents, MarkupContent, TextDocumentIdentifier, TextDocumentPositionParams,
        WorkDoneProgressParams, WorkspaceFolder,
    };
    use std::path::{Path, PathBuf};

    /// A unique temp dir per test (no `tempfile` dependency), mirroring the
    /// helper used in `config::tests`.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fpp-lsp-hover-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The `(line, character)` position (0-based) of the first occurrence of
    /// `needle` in `src`.
    fn position_of(src: &str, needle: &str) -> Position {
        let byte = src.find(needle).expect("needle present in source");
        let prefix = &src[..byte];
        let line = prefix.matches('\n').count() as u32;
        let col = prefix.len() as u32 - prefix.rfind('\n').map_or(0, |i| i as u32 + 1);
        Position {
            line,
            character: col,
        }
    }

    /// Build a workspace-indexed `GlobalState` for `src` written to `<dir>/m.fpp`,
    /// returning the state and the file URI. The full workspace-scan pipeline runs
    /// so the state machine analysis is populated exactly as at runtime.
    fn state_for(dir: &Path, src: &str) -> (GlobalState, Uri) {
        std::fs::write(dir.join(".fpp-lsp"), "scanWorkspace: true\n").unwrap();
        std::fs::write(dir.join("m.fpp"), src).unwrap();

        let root_uri = Uri::from_str(&crate::uri::from_file_path(dir).unwrap()).unwrap();
        let file_uri =
            Uri::from_str(&crate::uri::from_file_path(dir.join("m.fpp")).unwrap()).unwrap();

        let (sender, _receiver) = crossbeam_channel::unbounded();
        let workspace_folders = Some(vec![WorkspaceFolder {
            uri: root_uri,
            name: "test".to_string(),
        }]);
        let mut state = GlobalState::new(workspace_folders, sender, Default::default());

        // Drive the real indexing pipeline: discover the config, scan+parse the
        // workspace, then run semantic analysis.
        state.on_task(Task::ReloadWorkspace);
        state.on_task(Task::LoadFullWorkspace);
        state.on_task(Task::Analysis);

        (state, file_uri)
    }

    fn hover_at(state: &GlobalState, uri: &Uri, position: Position) -> String {
        let hover = handle_hover(
            state,
            HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        )
        .expect("hover ok")
        .expect("hover present");
        match hover.contents {
            HoverContents::Markup(MarkupContent { value, .. }) => value,
            other => panic!("expected markup hover, got {other:?}"),
        }
    }

    const SM_SRC: &str = "\
module M {
  state machine SM {
    action myAction
    guard myGuard
    signal mySignal
    initial enter Top
    state Top {
      choice Pick { if myGuard enter Top else enter Top }
    }
  }
}
";

    /// Hovering the name of a state machine member *definition* shows that
    /// member's own info (qualified under the state machine), not the enclosing
    /// state machine's. Regression test for the definition-name hover falling
    /// through to the parent `DefStateMachine`.
    #[test]
    fn hover_on_sm_member_definitions() {
        let dir = temp_dir("member-defs");
        let (state, uri) = state_for(&dir, SM_SRC);

        for (needle, expected) in [
            ("myAction", "(Action) SM.myAction"),
            ("myGuard", "(Guard) SM.myGuard"),
            ("mySignal", "(Signal) SM.mySignal"),
            ("Top", "(State) SM.Top"),
            ("Pick", "(Choice) SM.Top.Pick"),
        ] {
            let value = hover_at(&state, &uri, position_of(SM_SRC, needle));
            assert!(
                value.contains(expected),
                "hover on `{needle}` should contain `{expected}`, got:\n{value}"
            );
            assert!(
                !value.contains("(State Machine)"),
                "hover on `{needle}` should not show the parent state machine, got:\n{value}"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
