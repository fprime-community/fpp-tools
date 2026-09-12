use crate::owned::{OwnedDiagnostic, OwnedDiagnosticMessage};
use annotate_snippets::{Annotation, AnnotationKind, Element, Group, Snippet};
use fpp_core::{DiagnosticDataSnippet, DiagnosticMessageKind, Level};

fn diagnostic_level<'a>(level: Level) -> annotate_snippets::Level<'a> {
    match level {
        Level::Error => annotate_snippets::Level::ERROR,
        Level::Warning => annotate_snippets::Level::WARNING,
        Level::Note => annotate_snippets::Level::NOTE,
        Level::Help => annotate_snippets::Level::HELP,
        _ => annotate_snippets::Level::INFO,
    }
}

fn diagnostic_snippet_to_annotation<'a>(
    message: String,
    kind: AnnotationKind,
    snippet: &DiagnosticDataSnippet,
) -> Annotation<'a> {
    kind.span((snippet.start as usize)..(snippet.end as usize))
        .label(message)
}

/// The annotated source excerpt for one span of a diagnostic, followed by a note
/// per `include` that spliced its source in.
fn snippet_elements<'a>(
    message: String,
    kind: AnnotationKind,
    snippet: &DiagnosticDataSnippet,
) -> Vec<Element<'a>> {
    let excerpt = Snippet::source(snippet.file_content.clone())
        .line_start(snippet.line_offset + 1)
        .path(snippet.uri.clone())
        .annotation(diagnostic_snippet_to_annotation(message, kind, snippet));

    std::iter::once(Element::from(excerpt))
        .chain(snippet.include_spans.iter().map(|include_loc| {
            Element::Message(annotate_snippets::Level::NOTE.message(format!(
                "included from {}:{}:{}",
                include_loc.uri,
                include_loc.line + 1,
                include_loc.column + 1
            )))
        }))
        .collect()
}

fn child_elements<'a>(level: Level, child: &OwnedDiagnosticMessage) -> Vec<Element<'a>> {
    match &child.snippet {
        None => vec![Element::Message(
            (match child.kind {
                DiagnosticMessageKind::Primary => diagnostic_level(level),
                DiagnosticMessageKind::Note => diagnostic_level(Level::Note),
            })
            .message(child.message.clone()),
        )],
        Some(snippet) => snippet_elements(
            child.message.clone(),
            match child.kind {
                DiagnosticMessageKind::Primary => AnnotationKind::Primary,
                DiagnosticMessageKind::Note => AnnotationKind::Context,
            },
            snippet,
        ),
    }
}

pub(crate) fn diagnostic_to_snippet_group(diagnostic: &'_ OwnedDiagnostic) -> Group<'_> {
    let level = diagnostic_level(diagnostic.level);
    let group = match &diagnostic.snippet {
        None => Group::with_title(level.primary_title(diagnostic.message.clone())),
        Some(snippet) => Group::with_level(level).elements(snippet_elements(
            diagnostic.message.clone(),
            AnnotationKind::Primary,
            snippet,
        )),
    };

    group.elements(
        diagnostic
            .children
            .iter()
            .flat_map(|child| child_elements(diagnostic.level, child)),
    )
}
