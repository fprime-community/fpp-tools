/// A stack of scopes, from outermost to innermost. Each level is identified by
/// the thing whose scope is open there.
#[derive(Debug, Clone)]
pub struct GenericNestedScope<L>(Vec<L>);

impl<L: Clone> GenericNestedScope<L> {
    /// Create a nested scope containing only the outermost scope
    pub fn new(global: L) -> Self {
        Self(vec![global])
    }

    /// Create a nested scope from a list of levels, outermost first
    pub fn from_levels(levels: Vec<L>) -> Self {
        Self(levels)
    }

    /// Push a new scope onto the stack
    pub fn push(&mut self, level: L) {
        self.0.push(level);
    }

    /// Pop a scope off the stack
    pub fn pop(&mut self) {
        self.0.pop();
    }

    /// Look up a symbol in all the scopes nested in this scope
    pub fn search<S, F: Fn(&L) -> Option<S>>(&self, predicate: F) -> Option<S> {
        // Work in the current scope and work out to the outermost
        self.0.iter().rev().find_map(predicate)
    }

    /// Get the innermost scope
    pub fn current(&self) -> &L {
        self.0.last().unwrap()
    }

    /// Get the outermost (global) scope
    pub fn global(&self) -> &L {
        self.0.first().unwrap()
    }
}
