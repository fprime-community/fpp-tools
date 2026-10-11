passive component C {
  dictionary type T = U32
  dictionary   constant c = 1
  dictionary array A = [2] U32
  dictionary enum E { X }
  dictionary struct S { x: U32 }
}

locate dictionary constant c at "c.fpp"
locate   dictionary type T at "t.fpp"
