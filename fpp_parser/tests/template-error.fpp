# A template argument must begin with 'constant', 'instance', or 'type'
expand T(1)
expand T(U32)
expand T(constant 1, 2)

# A constant template argument must have an expression
expand T(constant)

# An expansion must name a template
expand

# A template parameter must begin with 'constant', 'instance', or 'type'
module template T1(c: U32) {}

# A constant template parameter must have a type
module template T2(constant c) {}

# An instance template parameter must have an interface
module template T3(instance i) {}

# A type template parameter may not have a type
module template T4(type Ty: U32) {}
