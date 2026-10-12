---- MODULE SequenceAppendTypeError ----
EXTENDS Sequences
VARIABLE x
Init==x = TRUE
Next==UNCHANGED x
Use(F(_,_))==[]<>(F(x,TRUE)= <<TRUE>>)
Live==Use(Append)
====
