---- MODULE NativeHeadEmptyError ----
EXTENDS Sequences
VARIABLE x
Init==x=<<>>
Next==UNCHANGED x
Use(F(_))==[]<>F(x)
Live==Use(Head)
====
