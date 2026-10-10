module A { module B { module C { constant d = 0 } } }

@ The qualifier of a four-part name keeps its order
locate constant A.B.C.d at "wrong.fpp"
