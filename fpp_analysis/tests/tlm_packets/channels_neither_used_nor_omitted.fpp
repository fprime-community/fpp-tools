deployment topology T {

  instance c1

  instance c2

  instance c3

  telemetry packets P {

    packet P group 0 {
      c2.T
    }

  }

}
