# The ports of an ambiguous pattern are listed in the order the component
# declares them
module Fw {
  port Time
}
module M {
  passive component Time {
    sync input port timeGetIn2: Fw.Time
    sync input port timeGetIn1: Fw.Time
  }
  passive component C {
    time get port timeGetOut
  }
  instance $time: Time base id 0x100
  instance c: C base id 0x100
  topology T {
    instance $time
    instance c
    time connections instance $time { c }
  }
}
