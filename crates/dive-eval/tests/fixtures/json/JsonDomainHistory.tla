---- MODULE JsonDomainHistory ----
EXTENDS Naturals, TLC, Json
VARIABLE x
Unused == [a |-> 0]
Record == ToJson([i \in {[b |-> 1, a |-> 2]} |-> 7])
Tuple == ToJson([i \in {<<[b |-> 1, a |-> 2]>>} |-> 7])
Set == ToJson([i \in {{[b |-> 1, a |-> 2]}} |-> 7])
Function == ToJson([i \in {[j \in {0} |-> [b |-> 1, a |-> 2]]} |-> 7])
Predicate == Record = "{\"[b |-> 1, a |-> 2]\":7}"
Init == x = 0
Next == x' = x
====
