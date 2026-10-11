# Rust-only: the implied uses of a string type name the framework definitions
# by their fully qualified names
locate type FwSizeStoreType at "implied_absolute_size.fpp"
locate type M.FwSizeStoreType at "implied_absolute_m_size.fpp"
locate constant FW_FIXED_LENGTH_STRING_SIZE at "implied_absolute_len.fpp"
locate constant M.FW_FIXED_LENGTH_STRING_SIZE at "implied_absolute_m_len.fpp"

module M {
  type s = string
}
