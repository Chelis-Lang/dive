---- MODULE SequenceSubSeqConstantHolds ----
EXTENDS Sequences
VARIABLE x
Init==x = <<FALSE, TRUE>>
Next==UNCHANGED x
Use(F(_,_,_))==[]<>(F(<<FALSE, TRUE>> , 2, 2)= <<TRUE>>)
Live==Use(SubSeq)
====
