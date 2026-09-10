topology Inner {

  instance c1

}

deployment topology T {

  instance c1

  import Inner

  telemetry packets P {

    packet P1 group 0 {
      Inner.T
    }

  } omit {
    c1.T
  }

}
