---- MODULE StartupAssume ----
EXTENDS Naturals, FiniteSets
CONSTANTS Nodes, Want
VARIABLE x

\* As in Examples MisraReachability: only the ASSUME reads Test, which
\* applies a parametrized instance to every graph. The core evaluates both
\* when it compiles its plan; the reference evaluator checks the ASSUME
\* only when the core does not find it TRUE.
G(Succ) == INSTANCE StartupAssumeGraph
Graphs == [Nodes -> SUBSET Nodes]
Test == << \A s \in Graphs : G(s)!Closed, Cardinality(Graphs) >>
ASSUME Test[1] /\ Test[2] = Want

Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == x <= 2
====
