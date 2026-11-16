//! The part of mago-reporting that mago-syntax uses: `Issue` as a plain builder. The real crate
//! needs schemars with `preserve_order`, which changes the key order of every `serde_json` map in
//! the build.

use mago_span::Span;

#[derive(Debug, Clone)]
pub struct Annotation {
    pub span: Span,
    pub message: Option<String>,
}

impl Annotation {
    pub fn primary(span: Span) -> Self {
        Annotation {
            span,
            message: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub message: String,
    pub code: Option<String>,
    pub annotations: Vec<Annotation>,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

impl Issue {
    pub fn error(message: impl Into<String>) -> Self {
        Issue {
            message: message.into(),
            code: None,
            annotations: Vec::new(),
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_annotation(mut self, annotation: Annotation) -> Self {
        self.annotations.push(annotation);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}
