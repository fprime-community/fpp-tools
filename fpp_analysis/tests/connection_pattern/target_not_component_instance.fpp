port P

passive component C {

  sync input port pIn: P

}

instance c: C base id 0x100

topology Inner {

  instance c

}

topology T {

  instance c

  import Inner

  health connections instance c {

    Inner

  }

}
