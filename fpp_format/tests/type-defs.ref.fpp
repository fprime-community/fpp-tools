@ Type-definition variants: abstract types, aliases, enum rep types, string sizes
module Types {
    @ Abstract type (no `=`)
    type AbstractType

    @ Type alias to a primitive
    type AliasU32 = U32

    @ Type alias to a qualified name
    type AliasQual = NS.SomeType

    @ Type alias to a sized string
    type SizedStringAlias = string size 40

    @ Enum with explicit representation type
    enum RepEnum : U8 {
        A = 0
        B = 1
        C = 2
    }

    @ Enum with rep type and default
    enum DefaultedEnum : I32 {
        X
        Y
        Z
    } default Y

    @ Struct with a sized-string field
    struct Message {
        header: U32
        body: string size 128
    }

    @ Array of sized strings
    array Names = [4] string size 16

    @ Array with default and format together
    array Levels = [3] U8 default 0 format "level {}"

    @ Dictionary-prefixed definitions
    dictionary constant DICT_CONST = 7
    dictionary type DictType = U32
    dictionary enum DictEnum {
        A
        B
    }
    dictionary array DictArray = [2] U32
    dictionary struct DictStruct {
        x: U32
    }
}
