---- MODULE SequenceSubSeqLetFails ----
EXTENDS Sequences
VARIABLE x
Init==x = <<FALSE, TRUE>>
Next==UNCHANGED x
Use(F(_,_,_))==[]<>(~(F(x, 2, 2)= <<TRUE>>))
Live==LET Pass(G(_,_,_))==Use(G) IN Pass(SubSeq)
====
