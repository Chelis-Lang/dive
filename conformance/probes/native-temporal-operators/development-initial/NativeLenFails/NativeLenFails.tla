---- MODULE NativeLenFails ----
EXTENDS Sequences
VARIABLE x
Init == x = <<1>>
Next == UNCHANGED x
Use(F(_)) == []<>(F(x) = 2)
Live == Use(Len)
====
