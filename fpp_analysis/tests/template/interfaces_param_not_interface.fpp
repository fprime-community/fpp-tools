port P

module M {}

active component A {
    async input port aIn: P
}

instance a: A base id 0x100 queue size 10 stack size 1024

# M is a module, not an interface
module template T(instance i: M) {
    topology Top {
        instance i
    }
}

module N {
    expand T(instance a)
}
