---- MODULE StoredRecordFails ----

VARIABLE x
Init == x = [flag |-> TRUE, bag |-> {1}]
Next == UNCHANGED x
Live == []<>(~(x.flag))
====
