---- MODULE SequenceTailStateHolds ----
EXTENDS Sequences
VARIABLE x
Init==x = <<FALSE, TRUE>>
Next==UNCHANGED x
Use(F(_))==[]<>(F(x)= <<TRUE>>)
Live==Use(Tail)
====
