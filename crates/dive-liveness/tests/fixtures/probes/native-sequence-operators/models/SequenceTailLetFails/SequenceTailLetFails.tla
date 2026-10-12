---- MODULE SequenceTailLetFails ----
EXTENDS Sequences
VARIABLE x
Init==x = <<FALSE, TRUE>>
Next==UNCHANGED x
Use(F(_))==[]<>(~(F(x)= <<TRUE>>))
Live==LET Pass(G(_))==Use(G) IN Pass(Tail)
====
