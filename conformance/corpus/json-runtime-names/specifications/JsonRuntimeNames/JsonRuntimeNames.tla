---- MODULE JsonRuntimeNames ----
EXTENDS Naturals, Json
VARIABLES n, s
Init == /\ n = 0 /\ s = ToJson(<<n>>)
Next == /\ n < 2 /\ n' = n + 1 /\ s' = ToJson(<<n'>>)
TypeOK == /\ n \in 0..2 /\ s = ToJson(<<n>>)
====
