---- MODULE EmptyRecordQuery ----
VARIABLE x
Init == x={}
Next == /\ [k|->1] \notin x /\ UNCHANGED x
====
