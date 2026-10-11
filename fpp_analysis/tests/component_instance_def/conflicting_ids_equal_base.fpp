module Fw {
  port Tlm
  port Time
}

passive component C {

  telemetry port tlmOut

  time get port timeGetOut

  telemetry T: U32

}

instance a: C base id 0x100

instance b: C base id 0x100

instance c: C base id 0x100
