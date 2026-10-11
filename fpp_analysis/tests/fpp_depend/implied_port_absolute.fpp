# Rust-only: the implied use of a special port instance names the framework
# port by its fully qualified name
locate port Fw.Cmd at "implied_port_absolute_fw_cmd.fpp"
locate port M.Fw.Cmd at "implied_port_absolute_m_fw_cmd.fpp"

module M {
  passive component C {
    command recv port cmdIn
  }
}
