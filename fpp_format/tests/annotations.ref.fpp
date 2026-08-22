# Standalone comment at module level
@ Pre-annotation for module
module AnnotatedModule {
    # Comment inside module

    @ Pre-annotation for constant
    constant MAX_VALUE = 100  @< Post-annotation for constant

    # Standalone comment before enum
    @ Enum with annotations
    enum Status {
        @ Active status
        ACTIVE = 0  @< Post-annotation for ACTIVE
        # Comment between enum members
        @ Idle status
        IDLE = 1  @< Post-annotation for IDLE
        STOPPED # inline comment
    } default IDLE  @< Post-annotation for enum

    # Comment before type
    @ Type alias annotation
    type MyType = U32  @< Post-annotation for type

    @ Array with annotations
    array DataArray = [50] F32 default 0.0  @< Post-annotation for array

    # Multiple standalone comments

    # Can appear in sequence

    @ Struct with member annotations
    struct Record {
        @ Field x annotation
        x: U32  @< Post-annotation for x
        # Comment between fields
        y: F32 # inline comment on field
        @ Field z annotation
        z: string  @< Post-annotation for z
    }  @< Post-annotation for struct

    @ Constant with inline and post
    constant MIN_VALUE = 0 # inline comment
    @< Post-annotation for constant

    # Final standalone comment
}  @< Post-annotation for module
