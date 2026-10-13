---- MODULE CdNext ----
\* Probe: `A \cdot B` as a whole action. TLC enumerates A's intermediate
\* states, then B from each, and counts the intermediate states as generated.
\* The ASSUME reads the switch as MCCRDT and MCReplicatedLog do.
EXTENDS Naturals, TLC
VARIABLES x, y
ASSUME TLCGet("-Dtlc2.tool.impl.Tool.cdot") = "true"
Init == x = 0 /\ y = 0
A == x < 4 /\ x' \in {x + 1, x + 2} /\ y' = y
B == y < 3 /\ x' = x /\ y' \in {y, y + 1}
Next == A \cdot B \/ (x >= 4 /\ UNCHANGED <<x, y>>)
Inv == x <= 5
====
