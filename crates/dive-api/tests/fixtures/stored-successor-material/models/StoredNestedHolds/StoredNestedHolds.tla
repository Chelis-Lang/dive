---- MODULE StoredNestedHolds ----

VARIABLE x
Init == x = <<[flag |-> TRUE, bag |-> {"alpha"}]>>
Next == UNCHANGED x
Live == []<>(x[1].flag)
====
