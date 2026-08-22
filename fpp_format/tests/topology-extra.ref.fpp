@ Topology constructs not in the main topology fixture
module TopExtra {
    @ Topology implementing interfaces
    topology Impl implements Base.TopoA, Base.TopoB {
        instance compA
        instance compB
    }

    @ Pattern connections with explicit target lists
    topology Patterns {
        instance central
        instance a
        instance b

        @ Command pattern targeting a specific list of instances
        command connections instance central {
            a
            b
        }

        @ Health pattern with targets
        health connections instance central {
            a
            b
        }

        @ Param pattern with targets
        param connections instance central {
            a
        }

        @ Telemetry packet set with includes and omit
        telemetry packets PktSet {
            include "packets/Common.fppi"
            packet Pkt1 id 1 group 2 {
                include "packets/Ch.fppi"
                central.channelA
            }
        } omit {
            central.debugChannel
        }
    }
}
