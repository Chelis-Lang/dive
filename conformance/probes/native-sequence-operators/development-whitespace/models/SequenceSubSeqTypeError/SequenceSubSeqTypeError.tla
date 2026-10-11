---- MODULE SequenceSubSeqTypeError ----
EXTENDS Sequences
VARIABLE x
Init==x=TRUE
Next==UNCHANGED x
Use(F(_,_,_))==[]<>(F(x,1,1)=<<TRUE>>)
Live==Use(SubSeq)
====
