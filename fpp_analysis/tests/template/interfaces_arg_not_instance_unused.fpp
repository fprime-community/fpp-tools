interface I {}

module M {}

# No topology uses the parameter
module template T(instance i: I) {}

# M is a module, not a component instance or topology
expand T(instance M)
