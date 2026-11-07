---- MODULE HoStdErr ----
\* An error inside a standard operator applied by a Java override: TLC
\* wraps it in the override's report, which the core refuses.
EXTENDS Naturals, Functions
VARIABLE x
Init == x = 0
Next == x' = x
Inv == FoldFunctionOnSet(+, 0, [i \in {1} |-> "a"], {1}) = 0
====
