# The queue size of a component instance must have a numeric type
active component C { async input port p: P }
port P
instance c: C base id 0x100 queue size true
