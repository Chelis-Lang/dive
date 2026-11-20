---- MODULE SymLoop ----
\* x alternates between two symmetric model values. Under SYMMETRY the second
\* state is equivalent to the first, though x = b holds only there.
EXTENDS TLC
CONSTANTS a, b
VARIABLE x
Init == x = a
Next == x' = IF x = a THEN b ELSE a
Spec == Init /\ [][Next]_x
FairSpec == Spec /\ WF_x(Next)
EventuallyB == <>(x = b)
InfOftenA == []<>(x = a)
NeverOther == <>(x \notin {a, b})
Perms == Permutations({a, b})
NotB == x # b
====
