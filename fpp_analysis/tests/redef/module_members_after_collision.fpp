@ The module name collides with the component, so the module's name is rejected
@ and nothing can reach it. Its members are still entered and analyzed, and the
@ module still has a scope of its own, so the later passes that walk them from
@ the AST find the symbol table they expect.
passive component DpCfg {
}
module DpCfg {
  constant X = 1
  array Arr = [2] U8
  struct S { x: U8 }
  enum E { P }
  type T = U8
  module Inner {
    constant Y = 2
  }
}
