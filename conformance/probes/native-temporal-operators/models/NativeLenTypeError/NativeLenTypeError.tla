---- MODULE NativeLenTypeError ----
EXTENDS Sequences
VARIABLE x
Init==x=TRUE
Next==UNCHANGED x
Use(F(_))==[]<>(F(x)=1)
Live==Use(Len)
====
