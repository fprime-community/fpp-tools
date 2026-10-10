# Template forms beyond the ones the copy of fpp-syntax/test/template.fpp covers

@ A template with no parameter list
module template NoParams {
    constant k = 1
}

@ A template with an empty parameter list
module template EmptyParams() {
    constant k = 1
}

@ An expansion with no argument list
expand NoParams

@ An expansion with an empty argument list
expand EmptyParams()

@ A qualified template name
expand A.B.T(constant 1)

@ An argument list separated by newlines instead of commas
expand Tmpl(
    constant 1
    type U32
    instance inst1
)

@ Escaped reserved words as the names of a template, its parameters, and the
@ definitions in its body
module template $template(
    constant $constant: U32
    instance $instance: Interface
    type $type
) {
    constant $expand = $constant
}

expand $template(constant 1, instance inst1, type U32)

locate template Tmpl at "template.fpp"
locate template A.B.T at "template.fpp"
locate template $template at "template.fpp"
