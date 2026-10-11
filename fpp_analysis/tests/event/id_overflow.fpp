# Identifiers are kept as 128-bit integers. The default identifier after the
# largest one is out of that range, so it is reported as an error where Scala,
# with arbitrary-precision identifiers, assigns it.

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
