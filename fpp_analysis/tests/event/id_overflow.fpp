# Identifiers are kept as 128-bit integers. Event E has the largest one, so the
# default identifier after it, which event F would get, is out of that range:
# the error is reported at E.

module Fw {
  port Log
  port LogText
  port Time
}

passive component C {

  event port eventOut

  text event port textEventOut

  time get port timeGetOut

  event E severity activity low id 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF format ""

  event F severity activity low format ""

}
