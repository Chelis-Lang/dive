---- MODULE NativeHeadLetFails ----
EXTENDS Sequences
VARIABLE x
Init==x = <<FALSE>>
Next==UNCHANGED x
Use(F(_))==[]<>F(x)
Live==LET Pass(G(_))==Use(G) IN Pass(Head)
====
