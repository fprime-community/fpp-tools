@ Interfaces, interface imports, and module-level includes
module Interfaces {
    @ Module-level include
    include "common/Ports.fppi"

    @ Empty interface
    interface EmptyInterface {
    }

    @ Interface with imports and port instances
    interface DataInterface {
        @ Import another interface
        import Base.CommonInterface
        import OtherInterface

        @ Port instances inside the interface
        async input port dataIn: DataPort
        output port dataOut: DataPort
    }

    @ Interface importing several
    interface CombinedInterface {
        import DataInterface
        import Interfaces.EmptyInterface
    }
}
