//! Rule diagnostics as LSP diagnostics.

use crate::analysis::Cancel;
use crate::notifier::Notifier;
use crate::position::Positions;
use crate::server::path_of;
use crate::workspace::Workspace;
use core_incremental::{
    Failure, Stage, directory_scope, lint, module_diagnostics, project_diagnostics,
};
use core_rules::fingerprint::new_findings;
use core_rules::{Diagnostic, Severity};
use lsp_types::{
    DiagnosticRelatedInformation, DiagnosticSeverity, DocumentDiagnosticParams,
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, FullDocumentDiagnosticReport,
    Location, NumberOrString, RelatedFullDocumentDiagnosticReport, Uri,
};
use std::path::Path;

pub const SOURCE: &str = "sudsy";

pub(crate) fn to_lsp(
    d: &Diagnostic,
    positions: &Positions<'_>,
    uri: &Uri,
) -> lsp_types::Diagnostic {
    let related: Vec<_> = d
        .related
        .iter()
        .map(|r| DiagnosticRelatedInformation {
            location: Location::new(
                uri.clone(),
                positions.range(r.range.start().into(), r.range.end().into()),
            ),
            message: r.message.clone(),
        })
        .collect();
    lsp_types::Diagnostic {
        range: positions.range(d.range.start().into(), d.range.end().into()),
        severity: Some(match d.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        code: Some(NumberOrString::String(d.rule.clone())),
        source: Some(SOURCE.to_string()),
        message: d.message.clone(),
        related_information: (!related.is_empty()).then_some(related),
        ..Default::default()
    }
}

impl Workspace {
    /// Why the semantic rules had nothing to look at in an open document, if they did not.
    pub fn semantic_warning(&self, path: &Path) -> Option<Failure> {
        let linted = lint(&self.db, self.file(path)?).as_ref().ok()?;
        Some(Failure {
            stage: Stage::Semantics,
            message: format!(
                "semantic rules are skipped: {}",
                linted.semantics_unavailable.as_ref()?
            ),
        })
    }

    /// The diagnostics of an open document; none for a document that is not open. A failure is
    /// why the rules could not run (config, worker).
    pub fn diagnostics(
        &self,
        path: &Path,
        uri: &Uri,
    ) -> Result<Vec<lsp_types::Diagnostic>, Failure> {
        let Some(file) = self.file(path) else {
            return Ok(Vec::new());
        };
        let linted = lint(&self.db, file).as_ref().map_err(Clone::clone)?;
        let text = file.text(&self.db);
        let mut found: Vec<&Diagnostic> = linted.diagnostics.iter().collect();
        // The directory rules judge the unit of the file's folder: its other files are the open
        // documents and what the disk holds.
        let module;
        if let Some(scope) = directory_scope(&self.db, file)
            && let Some(dir) = path.parent()
        {
            let open: Vec<_> = self.project_open_in(dir);
            let unit = self.siblings.unit(&self.db, dir, &scope, &open);
            module = module_diagnostics(&self.db, unit)
                .as_ref()
                .map_err(Clone::clone)?;
            found.extend(module.iter().filter(|(p, _)| p == path).map(|(_, d)| d));
        }
        // `analysis: deep`: what the project-scope rules found in this file.
        let project_wide;
        if self.is_deep() {
            let files = self.project_files();
            project_wide = project_diagnostics(&self.db, &files)?;
            found.extend(
                project_wide
                    .iter()
                    .filter(|(p, _)| p == path)
                    .map(|(_, d)| d),
            );
        }
        if let Some(head) = &self.head {
            let base = head.at_head(&self.db, path, file);
            let base = match base {
                Some(at) => Some((
                    at.text(&self.db),
                    lint(&self.db, at).as_ref().map_err(Clone::clone)?,
                )),
                None => None,
            };
            let base = base.map(|(t, l)| (t.as_str(), l.diagnostics.as_slice()));
            let mut new = new_findings(text, found.iter().copied(), base).into_iter();
            found.retain(|_| new.next().unwrap_or(true));
        }
        let positions = Positions::new(text);
        Ok(found.iter().map(|d| to_lsp(d, &positions, uri)).collect())
    }
}

/// `textDocument/diagnostic` on a snapshot, on the analysis thread.
pub(crate) fn diagnostic(
    workspace: &Workspace,
    notifier: &Notifier,
    params: DocumentDiagnosticParams,
    cancel: &Cancel,
) -> DocumentDiagnosticReportResult {
    let uri = params.text_document.uri;
    let path = path_of(&uri);
    if let Some(failure) = path.as_deref().and_then(|p| workspace.semantic_warning(p)) {
        notifier.failed(&uri, &failure);
    }
    // Checkpoint: the first query above is the long one; a result nobody wants is not built.
    let items = match path.filter(|_| cancel.error().is_none()) {
        Some(p) => match workspace.diagnostics(&p, &uri) {
            Ok(items) => items,
            Err(failure) => {
                notifier.failed(&uri, &failure);
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
        RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: None,
                items,
            },
        },
    ))
}
