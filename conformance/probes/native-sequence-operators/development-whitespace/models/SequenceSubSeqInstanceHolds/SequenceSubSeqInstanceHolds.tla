---- MODULE SequenceSubSeqInstanceHolds ----
EXTENDS Sequences
VARIABLE x
S==INSTANCE Sequences
Init==x=<<FALSE, TRUE>>
Next==UNCHANGED x
Use(F(_,_,_))==[]<>(F(x, 2, 2)=<<TRUE>>)
Live==Use(S!SubSeq)
====
