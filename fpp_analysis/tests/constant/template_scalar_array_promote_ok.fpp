array B = [2] U32

struct S { b: B }

module template T(constant b: B, constant s: S) {
  constant inner = [ 1, b ]
  constant outer = [ inner, inner ]
  constant x = outer[0][0][1]
  constant y = [ inner[0], inner[0] ]
  constant z = y[0][1]
  constant innerStruct = [ 1, s.b ]
  constant outerStruct = [ innerStruct, innerStruct ]
  constant w = outerStruct[0][0][1]
}

module M {
  expand T(constant [2, 3], constant { b = [4, 5] })
}
