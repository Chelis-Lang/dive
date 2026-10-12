---- MODULE StoredTupleHolds ----

VARIABLE x
Init == x = <<{1}, TRUE>>
Next == UNCHANGED x
Live == []<>(x[2])
====
