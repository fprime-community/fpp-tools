use fpp_core::{DiagnosticData, DiagnosticEmitter, Level};
use fpp_errors::{OwnedDiagnostic, Renderer, renderer};
use std::io::Write;

pub struct StderrEmitter {
    renderer: Renderer,
    seen_errors: bool,
}

impl StderrEmitter {
    pub fn new() -> StderrEmitter {
        StderrEmitter {
            renderer: renderer(true),
            seen_errors: false,
        }
    }

    pub fn has_errors(&self) -> bool {
        self.seen_errors
    }
}

impl Default for StderrEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticEmitter for &mut StderrEmitter {
    fn emit(&mut self, diagnostic: DiagnosticData) {
        if diagnostic.level == Level::Error {
            self.seen_errors = true;
        }
        let rendered = OwnedDiagnostic::from(&diagnostic).render(&self.renderer);
        let mut stderr = anstream::stderr();
        let _ = writeln!(stderr, "{rendered}\n");
    }
}
