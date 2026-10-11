---- MODULE SequenceAppendConstantHolds ----
EXTENDS Sequences
VARIABLE x
Init==x=<<TRUE>>
Next==UNCHANGED x
Use(F(_,_))==[]<>(F(<<TRUE>> , TRUE)=<<TRUE, TRUE>>)
Live==Use(Append)
====
