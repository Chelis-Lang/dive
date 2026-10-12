---- MODULE NativeInstanceHolds ----
VARIABLE x
S==INSTANCE Sequences
Init==x = <<TRUE>>
Next==UNCHANGED x
Use(F(_))==[]<>F(x)
Live==Use(S!Head)
====
