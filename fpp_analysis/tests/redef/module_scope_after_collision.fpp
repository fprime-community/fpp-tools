@ A member of the rejected module refers to another one, so the module's scope
@ has to exist and hold both.
constant DpCfg = 1
module DpCfg {
  constant X = 1
  constant Y = X
}
