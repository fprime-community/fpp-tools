port P

interface I {
    output port pOut: P
}

module M {}

module template T(instance i: I) {
    topology Top {
        instance i
    }
}

# M is a module, not a component instance or topology
expand T(instance M)
