port Fw
port P
passive component C { time get port timeGetOut }
passive component D { sync input port pIn: P }
instance c: C base id 0x100
instance d: D base id 0x200
topology T { instance c; instance d; connections X { c.timeGetOut -> d.pIn } }
