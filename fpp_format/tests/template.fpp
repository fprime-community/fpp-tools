interface Iface {

}

@ Template Definition
module template T (
    constant c: U32, @< A constant parameter
    type Ty, @< A type parameter
    instance inst: Iface @< An interface parameter
) {
    port P(
        a: Ty
    ) -> Ty

    constant d = c + 2
}

@ The expansion specifier is formatted but not expanded
expand T(constant 10, type U32, instance inst1)

@ A template with no parameter list
module template NoParams {
  constant k = 1
}

@ A template with an empty parameter list
module template EmptyParams () {
  constant k = 1
}

expand NoParams
expand EmptyParams ( )
expand A.B.T(constant 1 + 2, instance a.b, type U32)

@ Template parameters and arguments whose names are escaped keywords:
@ the escapes must survive formatting so the output still parses
module template Escaped(
    constant $type: U32, @< An escaped constant parameter
    type $array, @< An escaped type parameter
    instance $instance: Iface @< An escaped instance parameter
) {
    array a = [3] $array

    constant y = $type + 1

    module $module {
        constant $constant = 1
    }

    constant z = $module.$constant
}

expand Escaped(constant 5, type U32, instance $instance)

locate template T at "template.fpp"
locate template $template at "template.fpp"
