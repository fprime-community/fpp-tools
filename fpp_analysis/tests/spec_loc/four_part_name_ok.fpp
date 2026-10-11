module A { module C { module B { constant d = 0 } } }

@ No definition has this name, so nothing checks its location
locate constant A.B.C.d at "wrong.fpp"
