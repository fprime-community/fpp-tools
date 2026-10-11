# Rust-only: a deployment topology copied by a template expansion is visited
# at the expansion
module template T {
  deployment topology A {
    instance c2
    telemetry packets Q {
      packet Q group 0 { c2.T }
      packet Q group 1 { c2.T }
    }
  }
}
deployment topology B {
  instance c1
  telemetry packets P {
    packet P group 0 { c1.T }
    packet P group 1 { c1.T }
  }
}
module M { expand T }
