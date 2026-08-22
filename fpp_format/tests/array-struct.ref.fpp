@ Arrays and structs with various formatting
module DataTypes {
    @ Simple array
    array SimpleArray = [10] U32

    @ Array with default
    array DefaultArray = [5] F32 default 0.0

    @ Array with format
    array FormattedArray = [20] U32 default 100 format "{} units"

    @ String array
    array StringArray = [3] string

    @ Multi-dimensional via nested arrays
    array MatrixRow = [10] F32
    array Matrix = [5] MatrixRow

    @ Simple struct
    struct Point {
        x: U32
        y: U32
    }

    @ Struct with format on fields
    struct Measurement {
        value: F32 format "{} meters"
        timestamp: U32
        unitCode: U8
    }

    @ Struct with array field
    struct Data {
        samples: [100] F32
        count: U32
    }

    @ Struct with default block
    struct Config {
        enabled: bool
        timeout: U32
        maxRetries: U8
    } default { enabled = true, timeout = 1000, maxRetries = 3 }

    @ Nested struct
    struct SystemState {
        position: Point
        measurement: Measurement
    } default {
        position = { x = 0, y = 0 }
        measurement = { value = 0.0, timestamp = 0, unitCode = 1 }
    }
}
