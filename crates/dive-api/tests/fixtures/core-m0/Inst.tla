---- MODULE Inst ----
\* INSTANCE substitutions in the core (`04-plan-compiler.md` §3, pass 2):
\* variables, constants and operators substituted, a parametrized instance,
\* split actions found inside instances, a lazy substituted argument, and
\* an expression substituted for a variable under a prime.
EXTENDS Naturals
VARIABLE x, y, z
Inc(a) == a + 1
I == INSTANCE InstInner WITH C <- 1, v <- x, F <- Inc
J(k) == INSTANCE InstInner WITH C <- k, v <- y, F <- Inc
K == INSTANCE InstInner WITH C <- 0, v <- x + z, F <- Inc
Init == I!Init /\ J(2)!Init /\ z = 0
Next == \/ I!Step /\ UNCHANGED <<y, z>>
        \/ J(2)!Step /\ UNCHANGED <<x, z>>
        \/ z < 1 /\ z' = z + 1 /\ UNCHANGED <<x, y>> /\ K!Grows
Inv == /\ I!Inv /\ J(2)!Inv /\ K!Inv
       /\ I!Twice = 2 * x
       /\ I!Lazy(1 \div 0) = x
Spec == Init /\ [][Next]_<<x, y, z>>
====
