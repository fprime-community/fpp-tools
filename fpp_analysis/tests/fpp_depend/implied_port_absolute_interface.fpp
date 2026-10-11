# Rust-only: the implied use of a special port instance in an interface names
# the framework port by its fully qualified name
locate port Fw.Cmd at "implied_port_absolute_interface_fw_cmd.fpp"
locate port M.Fw.Cmd at "implied_port_absolute_interface_m_fw_cmd.fpp"

module M {
  interface I {
    command recv port cmdIn
  }
}
