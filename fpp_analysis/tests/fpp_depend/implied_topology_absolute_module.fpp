# Rust-only: the implied uses of a deployment topology in a module name the
# framework definitions by their fully qualified names
locate type FwChanIdType at "implied_topology_absolute_module_chan.fpp"
locate type M.FwChanIdType at "implied_topology_absolute_module_m_chan.fpp"

module M {
  deployment topology T {}
}
