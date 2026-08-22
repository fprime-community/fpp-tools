@ Test topology
module TestTopology {
    @ Deployment topology
    deployment topology DeploymentTopology {
        instance compA
    }

    topology RefTopology {
        @ Instance imports and definitions
        import SubModule.SubTopology
        import AnotherModule.Topology

        instance compA
        instance compB
        instance compC

        @ Port exports
        port exportedPort = compA.outputPort

        @ Pattern connection graphs
        command connections instance compA

        event connections instance compB

        telemetry connections instance compC

        time connections instance compA

        param connections instance compB

        health connections instance compA

        text event connections instance compB

        @ Direct connections
        connections MainConnections {
            compA.dataOut    -> compB.dataIn
            compB.controlOut -> compC.controlIn
            compC.statusOut  -> compA.statusIn

            compA.portArray[0] -> compB.singlePort
            compB.dataOut[1]   -> compC.dataIn[1]

            unmatched compA.optionalOut -> compC.optionalIn
        }

        connections SecondaryConnections {
            compA.cmdOut -> compB.cmdIn
        }

        @ Telemetry packet set
        telemetry packets TestPackets {
            packet P1 group 1 {
                compA.channel1
                compB.channel2
                compC.channel3
            }
            packet P2 group 2 {
                compA.tempChannel
            }
        } omit {
            compA.debugChannel
            compB.internalChannel
        }
    }
}
