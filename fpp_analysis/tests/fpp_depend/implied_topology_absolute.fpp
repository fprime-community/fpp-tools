# Rust-only: the implied uses of a deployment topology name the framework
# definitions by their fully qualified names
locate type FwChanIdType at "implied_topology_absolute_chan.fpp"
locate type T.FwChanIdType at "implied_topology_absolute_t_chan.fpp"

deployment topology T {}
