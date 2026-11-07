---- MODULE StartupConstants ----
EXTENDS Naturals, FiniteSets, Sequences
CONSTANT Node
VARIABLES x, k

\* A simple cycle through every node, found by search; F[i-1] is applied
\* twice, so each candidate evaluates F exponentially often without the
\* core's call results.
IsCycle(S, r) ==
  LET F[i \in 1..Cardinality(S)] ==
        IF i = 1 THEN << CHOOSE s \in S : TRUE >>
        ELSE LET seq == F[i-1] IN << r[Head(seq)] >> \o F[i-1]
  IN {F[Cardinality(S)][j] : j \in 1..Cardinality(S)} = S

Ring == CHOOSE r \in [Node -> Node] : IsCycle(Node, r)
Start == CHOOSE n \in Node : TRUE
Bad == CHOOSE n \in Node : n # n
Limit == 2 * Cardinality(Node)

Init == x = Start /\ k = 0
Step == k < Limit /\ x' = Ring[x] /\ k' = k + 1
Next == Step
NextBad == Step \/ (k = Limit /\ x' = Bad /\ UNCHANGED k)
Inv == k <= Limit /\ x \in Node
\* Temporal properties: `Live` reaches no start-up definition, so the core
\* keeps all three; `Never` reads `Limit`, which the binder then evaluates.
Spec == Init /\ [][Next]_<<x, k>>
Live == <>[](x \in Node)
Never == <>(k > Limit)
====
