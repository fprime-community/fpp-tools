deployment topology T {

  instance c1

  instance c2

  instance c3

  telemetry packets P {

    packet P group 0 {
      c1.T
    }

  } omit {
    c1.T
    c3.T
  }

}
