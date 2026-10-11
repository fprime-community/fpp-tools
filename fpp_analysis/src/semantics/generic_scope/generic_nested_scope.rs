/// A type-generic stack of scopes
///
/// The stack runs from the outermost scope to the innermost one, and each level
/// is identified by the thing whose scope is open there. The scopes themselves
/// live in the analysis, which fills them in place while it enters symbols, so a
/// level names its scope instead of holding a copy of it. A lookup therefore
/// takes a function that finds the name in the scope of one level.
#[derive(Debug, Clone)]
pub struct GenericNestedScope<L>(Vec<L>);

impl<L: Clone> GenericNestedScope<L> {
    /// Create a nested scope holding only the outermost scope
    pub fn new(global: L) -> Self {
        Self(vec![global])
    }

    /// Push a new scope onto the stack
    pub fn push(&mut self, level: L) {
        self.0.push(level);
    }

    /// Pop a scope off the stack
    pub fn pop(&mut self) {
        self.0.pop().expect("empty scope stack");
    }

    /// Get a relative or absolute symbol from the nested scope
    pub fn get<S>(&self, is_absolute: bool, lookup: impl Fn(&L) -> Option<S>) -> Option<S> {
        match is_absolute {
            false => self.get_relative(lookup),
            true => self.get_absolute(lookup),
        }
    }

    /// Get a relative symbol from the nested scope. Start searching at the
    /// bottom of the stack, working upwards. Return None if the name is not there.
    pub fn get_relative<S>(&self, lookup: impl Fn(&L) -> Option<S>) -> Option<S> {
        self.0.iter().rev().find_map(lookup)
    }

    /// Get an absolute symbol from the nested scope. Start searching at
    /// the top of the stack. Return None if the name is not there
    pub fn get_absolute<S>(&self, lookup: impl Fn(&L) -> Option<S>) -> Option<S> {
        self.0.first().and_then(lookup)
    }

    /// Get the innermost nested scope
    pub fn inner_scope(&self) -> &L {
        self.0.last().expect("empty scope stack")
    }

    /// Get the outmost nested scope (the global scope)
    pub fn global_scope(&self) -> Self {
        Self(vec![self.0.first().expect("empty scope stack").clone()])
    }
}
