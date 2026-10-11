---- MODULE NativeHeadFails ----
EXTENDS Sequences
VARIABLE x
Init == x = <<FALSE>>
Next == UNCHANGED x
Use(F(_)) == []<>(F(x))
Live == Use(Head)
====
