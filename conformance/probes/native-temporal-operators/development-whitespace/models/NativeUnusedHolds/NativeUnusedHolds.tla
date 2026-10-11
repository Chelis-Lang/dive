---- MODULE NativeUnusedHolds ----
EXTENDS Sequences
VARIABLE x
Init==x=TRUE
Next==UNCHANGED x
Use(F(_))==[]<>x
Live==Use(Len)
====
