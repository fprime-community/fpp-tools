# A passive component instance may not have a queue size, whatever its type
passive component C { }
instance c: C base id 0 queue size "x"
