---- MODULE NativeLenAliasHolds ----
EXTENDS Sequences
VARIABLE x
Init==x = <<1>>
Next==UNCHANGED x
Use(F(_))==[]<>(F(x)=1)
LenAlias(v)==Len(v)
Live==Use(LenAlias)
====
