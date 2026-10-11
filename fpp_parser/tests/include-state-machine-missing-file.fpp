state machine S {
  include "this-file-should-not-exist.fppi"
  state A {
    include "this-file-should-not-exist-either.fppi"
  }
}
