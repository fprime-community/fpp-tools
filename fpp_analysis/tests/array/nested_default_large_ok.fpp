@ A nested array default. The element default is shared, so building this costs
@ the SUM of the nested sizes and not their PRODUCT.
array Inner = [4000] U8
array Outer = [4000] Inner

@ Sharing also holds through a struct member and a third level of nesting
array SmallInner = [8] U8
array SmallOuter = [8] SmallInner
struct Middle { i: SmallInner, o: SmallOuter }
array Deep = [8] Middle
