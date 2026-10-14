---- MODULE JsonSupported ----
EXTENDS Naturals, Sequences, TLC, Json
CONSTANT M
VARIABLE x
Actual == <<ToJson(1), ToJson(TRUE), ToJson(FALSE), ToJson(1..3), ToJson(<<"x", TRUE, <<1,2>>>>), ToJson(M), ToJson(<<>>), ToJson({3,1,2}), ToJson([i \in 1..2 |-> i]), ToJson([a |-> 1])>>
ASSUME PrintT(Actual)
Init == x = 0
Next == x' = x
====
