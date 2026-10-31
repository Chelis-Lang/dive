---- MODULE EmptySetQuery ----
VARIABLE x
Init == x={}
Next == /\ {1} \notin x /\ UNCHANGED x
====
