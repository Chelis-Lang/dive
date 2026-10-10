---- MODULE FinalAction ----
VARIABLE x
Init == x = 0
Next == x' \in {0,1,2}
Live == <>[][x' = 1]_x
====
