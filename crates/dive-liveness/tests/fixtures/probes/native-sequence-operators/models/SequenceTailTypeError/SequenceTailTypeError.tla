---- MODULE SequenceTailTypeError ----
EXTENDS Sequences
VARIABLE x
Init==x = TRUE
Next==UNCHANGED x
Use(F(_))==[]<>(F(x)= <<>>)
Live==Use(Tail)
====
