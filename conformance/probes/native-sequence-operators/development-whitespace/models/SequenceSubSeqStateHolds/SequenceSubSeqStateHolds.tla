---- MODULE SequenceSubSeqStateHolds ----
EXTENDS Sequences
VARIABLE x
Init==x=<<FALSE, TRUE>>
Next==UNCHANGED x
Use(F(_,_,_))==[]<>(F(x, 2, 2)=<<TRUE>>)
Live==Use(SubSeq)
====
