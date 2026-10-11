# The implied use Fw.Time of a time get port names a module, not a port.

module Fw {

  module Time {

  }

}

passive component C {

  time get port timeGetOut

}
