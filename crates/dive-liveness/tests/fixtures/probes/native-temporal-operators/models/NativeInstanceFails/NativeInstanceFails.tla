---- MODULE NativeInstanceFails ----
VARIABLE x
S==INSTANCE Sequences
Init==x = <<FALSE>>
Next==UNCHANGED x
Use(F(_))==[]<>F(x)
Live==Use(S!Head)
====
