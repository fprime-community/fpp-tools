# Rust-only: uses in the members of a module are looked up in the enclosing
# scopes
locate type M.E at "uses_in_nested_scopes_m_e.fpp"
locate type E at "uses_in_nested_scopes_e.fpp"
locate constant M.k at "uses_in_nested_scopes_m_k.fpp"
locate type M.S at "uses_in_nested_scopes_m_s.fpp"
locate type M.SM.T at "uses_in_nested_scopes_m_sm_t.fpp"
locate type T at "uses_in_nested_scopes_t.fpp"
locate state machine M.SM2 at "uses_in_nested_scopes_m_sm2.fpp"
locate instance M.inst at "uses_in_nested_scopes_m_inst.fpp"
locate component M.Comp at "uses_in_nested_scopes_m_comp.fpp"
locate interface M.I at "uses_in_nested_scopes_m_i.fpp"

module M {
  constant a = E.X
  array A = [3] S
  state machine SM {
    action act: T
    guard g: T
    signal s: T
    initial enter St
    state St
  }
  passive component C {
    state machine instance smi: SM2
    param p: U32 default k
  }
  topology Top implements I {
    instance inst
    connections Conn { inst.p -> inst.q }
  }
}
