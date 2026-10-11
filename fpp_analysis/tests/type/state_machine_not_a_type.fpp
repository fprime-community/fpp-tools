# A qualified identifier type name must refer to a type definition or a type
# parameter (FPP spec, Type Names). A state machine is in the type name group,
# but it defines no type, whether it is defined before or after the use.
state machine S { initial enter A; state A }
array T = [3] S
