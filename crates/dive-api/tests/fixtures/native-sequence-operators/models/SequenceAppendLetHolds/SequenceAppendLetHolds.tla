---- MODULE SequenceAppendLetHolds ----
EXTENDS Sequences
VARIABLE x
Init==x = <<TRUE>>
Next==UNCHANGED x
Use(F(_,_))==[]<>(F(x, TRUE)= <<TRUE, TRUE>>)
Live==LET Pass(G(_,_))==Use(G) IN Pass(Append)
====
