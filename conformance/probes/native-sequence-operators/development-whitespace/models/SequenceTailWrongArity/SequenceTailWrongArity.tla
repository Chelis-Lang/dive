---- MODULE SequenceTailWrongArity ----
EXTENDS Sequences
VARIABLE x
Init==x=TRUE
Next==UNCHANGED x
Use(F(_,_))==[]<>F(x,x)
Live==Use(Tail)
====
