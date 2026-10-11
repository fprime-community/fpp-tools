# The expressions of a component instance are checked with the instance, after
# the component definitions
active component C { }
instance c: C base id 0 cpu "bad"
