---- MODULE LazyUnionDup ----
\* UNION of a set enumeration with an interval member stays lazy
\* (UnionValue.union combines only set enumerations), so an element shared
\* by two members is listed twice: the core refuses.
EXTENDS Naturals
VARIABLE n
Init == n = 1
Next == n' \in UNION {{n}, 1..2}
====
