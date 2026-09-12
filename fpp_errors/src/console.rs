use crate::owned::{OwnedDiagnostic, renderer};
use annotate_snippets::Renderer;
use fpp_core::{DiagnosticData, Level};

pub struct ConsoleEmitter {
    renderer: Renderer,
    seen_errors: bool,
}

impl ConsoleEmitter {
    pub fn color() -> ConsoleEmitter {
        ConsoleEmitter {
            renderer: renderer(true),
            seen_errors: false,
        }
    }

    pub fn plain() -> ConsoleEmitter {
        ConsoleEmitter {
            renderer: renderer(false),
            seen_errors: false,
        }
    }

    pub fn has_errors(&self) -> bool {
        self.seen_errors
    }
}

impl fpp_core::DiagnosticEmitter for &mut ConsoleEmitter {
    fn emit(&mut self, diagnostic: DiagnosticData) {
        if diagnostic.level == Level::Error {
            self.seen_errors = true;
        }

        anstream::println!(
            "{}\n",
            OwnedDiagnostic::from(&diagnostic).render(&self.renderer)
        );
    }
}
