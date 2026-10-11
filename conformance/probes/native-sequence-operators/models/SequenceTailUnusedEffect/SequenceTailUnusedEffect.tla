---- MODULE SequenceTailUnusedEffect ----
EXTENDS Sequences, TLC
VARIABLE x
Unused==PrintT(TRUE)
Init==x = <<FALSE, TRUE>>
Next==UNCHANGED x
Use(F(_))==[]<>(F(x)= <<TRUE>>)
Live==Use(Tail)
====
