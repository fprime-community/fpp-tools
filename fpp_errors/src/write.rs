use crate::owned::{OwnedDiagnostic, renderer};
use annotate_snippets::Renderer;
use fpp_core::{DiagnosticData, DiagnosticEmitter};
use std::io::Write;

pub struct WriteEmitter<W: Write> {
    renderer: Renderer,
    write: W,
}

impl<W: Write> WriteEmitter<W> {
    pub fn new(w: W) -> WriteEmitter<W> {
        WriteEmitter {
            renderer: renderer(false),
            write: w,
        }
    }
}

impl<W: Write> DiagnosticEmitter for WriteEmitter<W> {
    fn emit(&mut self, diagnostic: DiagnosticData) {
        let mut out = OwnedDiagnostic::from(&diagnostic).render(&self.renderer);
        out.push('\n');
        out.push('\n');
        self.write
            .write_all(out.as_bytes())
            .expect("failed to write diagnostic");
    }
}
