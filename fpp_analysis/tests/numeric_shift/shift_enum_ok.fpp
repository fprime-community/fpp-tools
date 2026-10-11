@ Rust-only: shifts of enumerated constants
enum G: U8 { P = 3 }
enum H: U16 { Q = 2 }
constant kindPreserved = G.P << H.Q
constant kindDropped = G.P << 2
