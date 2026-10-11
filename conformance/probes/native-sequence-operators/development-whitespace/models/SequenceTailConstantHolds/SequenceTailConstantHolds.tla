---- MODULE SequenceTailConstantHolds ----
EXTENDS Sequences
VARIABLE x
Init==x=<<FALSE, TRUE>>
Next==UNCHANGED x
Use(F(_))==[]<>(F(<<FALSE, TRUE>>)=<<TRUE>>)
Live==Use(Tail)
====
