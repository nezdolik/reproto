//! Reporter for spanned diagnostics.
use flavored::RpName;
use std::fmt;
use std::slice;
use {Source, Span};

#[derive(Debug, Clone, Copy, Serialize)]
pub enum SymbolKind {
    #[serde(rename = "type")]
    Type,
    #[serde(rename = "interface")]
    Interface,
    #[serde(rename = "tuple")]
    Tuple,
    #[serde(rename = "enum")]
    Enum,
    #[serde(rename = "service")]
    Service,
}

/// A single diagnostic emitted by the compiler.
#[derive(Debug, Clone)]
pub enum Diagnostic {
    /// A positional error.
    Error { span: Span, message: String },
    /// A positional information string.
    Info { span: Span, message: String },
    /// A symbol that was encountered, and its location.
    Symbol {
        kind: SymbolKind,
        span: Span,
        name: RpName,
    },
}

/// A collection of diagnostics emitted by the compiler.
#[derive(Debug, Clone)]
pub struct Diagnostics {
    pub source: Source,
    pub items: Vec<Diagnostic>,
}

impl Diagnostics {
    /// Create a new diagnostics collection.
    pub fn new(source: Source) -> Self {
        Self {
            source,
            items: Vec::new(),
        }
    }

    /// Check if reporter is empty.
    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|item| match *item {
            Diagnostic::Error { .. } => true,
            _ => false,
        })
    }

    /// Report an error.
    pub fn err<S: Into<Span>, E: fmt::Display>(&mut self, span: S, error: E) {
        self.items.push(Diagnostic::Error {
            span: span.into(),
            message: error.to_string(),
        });
    }

    /// Report information.
    pub fn info<S: Into<Span>, I: fmt::Display>(&mut self, span: S, info: I) {
        self.items.push(Diagnostic::Info {
            span: span.into(),
            message: info.to_string(),
        });
    }

    /// Register a symbol.
    pub fn symbol<P: Into<Span>>(&mut self, kind: SymbolKind, span: P, name: &RpName) {
        self.items.push(Diagnostic::Symbol {
            kind,
            span: span.into(),
            name: name.clone(),
        });
    }

    /// Iterate over all reporter items.
    pub fn items(&self) -> Items {
        Items {
            iter: self.items.iter(),
        }
    }
}

/// A single diagnostic emitted by the compiler.
#[derive(Debug, Clone)]
pub enum SourceDiagnostic {
    /// A positional error.
    Error {
        source: Source,
        span: Span,
        message: String,
    },
    /// A positional information string.
    Info {
        source: Source,
        span: Span,
        message: String,
    },
    /// A symbol that was encountered, and its location.
    Symbol {
        kind: SymbolKind,
        source: Source,
        span: Span,
        name: RpName,
    },
}

/// A collection of diagnostics emitted by the compiler.
#[derive(Debug, Clone)]
pub struct SourceDiagnostics {
    pub items: Vec<(Source, Diagnostic)>,
}

impl SourceDiagnostics {
    /// Create a new sourced diagnostics collection.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Check if reporter is empty.
    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|item| match item.1 {
            Diagnostic::Error { .. } => true,
            _ => false,
        })
    }

    /// Report an error.
    pub fn err<S: Into<Span>, E: fmt::Display>(&mut self, source: &Source, span: S, error: E) {
        self.items.push((
            source.clone(),
            Diagnostic::Error {
                span: span.into(),
                message: error.to_string(),
            },
        ));
    }

    /// Report information.
    pub fn info<S: Into<Span>, I: fmt::Display>(&mut self, source: &Source, span: S, info: I) {
        self.items.push((
            source.clone(),
            Diagnostic::Info {
                span: span.into(),
                message: info.to_string(),
            },
        ));
    }

    /// Register a symbol.
    pub fn symbol<P: Into<Span>>(
        &mut self,
        kind: SymbolKind,
        source: &Source,
        span: P,
        name: &RpName,
    ) {
        self.items.push((
            source.clone(),
            Diagnostic::Symbol {
                kind,
                span: span.into(),
                name: name.clone(),
            },
        ));
    }

    /// Iterate over all reporter items.
    pub fn items(&self) -> SourceItems {
        SourceItems {
            iter: self.items.iter(),
        }
    }
}

/// Iterator over items.
///
/// Created using `Diagnostics::items`.
pub struct Items<'a> {
    iter: slice::Iter<'a, Diagnostic>,
}

impl<'a> Iterator for Items<'a> {
    type Item = <slice::Iter<'a, Diagnostic> as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Iterator over source items.
///
/// Created using `SourceDiagnostics::items`.
pub struct SourceItems<'a> {
    iter: slice::Iter<'a, (Source, Diagnostic)>,
}

impl<'a> Iterator for SourceItems<'a> {
    type Item = <slice::Iter<'a, (Source, Diagnostic)> as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
