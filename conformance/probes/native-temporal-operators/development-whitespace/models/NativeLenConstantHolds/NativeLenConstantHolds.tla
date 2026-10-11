---- MODULE NativeLenConstantHolds ----
EXTENDS Sequences
VARIABLE x
Init==x=TRUE
Next==UNCHANGED x
Use(F(_))==[]<>(F(<<1>>)=1)
Live==Use(Len)
====
