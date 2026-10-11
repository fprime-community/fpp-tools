# Each empty element sequence here holds a single separator

constant a = [,]
constant b = {,}
expand T(,)
state machine S {
  initial do { , } enter A
  state A
}
topology T implements , {
  connections C { , }
  command connections instance a { , }
  telemetry packets P {
    packet A group 0 { , }
  } omit { , }
}
passive component C {
  telemetry T: U32 low { , }
}
