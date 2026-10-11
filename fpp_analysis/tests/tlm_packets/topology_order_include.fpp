# Rust-only: a deployment topology in an included file is visited at the
# include specifier
deployment topology T1 {
  instance c1
  telemetry packets P {
    packet X group 0 {
    }
  }
}
include "include/topology_order.fppi"
