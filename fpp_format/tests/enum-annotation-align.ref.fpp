module M {
    @ Enum constants carrying both `=` values and `@<` annotations must align
    @ the `=` column and the `@<` column independently.
    enum TransactionType : U8 {
        BLOCK       = 0x0  @< One trigger required for each block transfer
        BEAT        = 0x2  @< One trigger required for each beat transfer
        TRANSACTION = 0x3  @< One trigger required for each transaction
    }

    @ Varying value widths: the annotation column follows the widest value.
    enum Widths {
        A    = 0x0    @< first
        BBBB = 0x200  @< second
        C    = 0x1    @< third
    }

    @ Enum without values still aligns the annotations alone.
    enum NoValues {
        OK         @< ok
        BUS_ERROR  @< error
    }

    @ Struct members align the `@<` annotation column too.
    struct Writeback {
        btctrl: U16    @< Block Transfer Control
        btcnt: U16     @< Remaining beat count
        descaddr: U32  @< Next descriptor address
    }
}
