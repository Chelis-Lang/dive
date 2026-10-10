---- MODULE SimActionProperty ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' \in {1, 2}
Act == [x' = x + 1]_x
Property == [][Act]_x
====
