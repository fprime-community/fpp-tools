# The port numbers of a connection are checked with the topology, after the
# component definitions
active component C { }
port P
passive component D {
  output port pOut: [2] P
  sync input port pIn: [2] P
}
instance d: D base id 0x100
topology T {
  instance d
  connections C1 { d.pOut["a"] -> d.pIn[0] }
}
