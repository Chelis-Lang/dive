---- MODULE FdRecursivePermFails ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
PermSeqs(S) ==
  LET perms[ss \in SUBSET S] ==
       IF ss = {} THEN {<<>>}
       ELSE LET ps == [n \in ss |-> {Append(sq,n) : sq \in perms[ss \ {n}]}]
            IN UNION {ps[n] : n \in ss}
  IN perms[S]
Domain == PermSeqs({1,2})
Init == x = TRUE
Next == UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == \A p \in Domain : []<>(x = (p[1] = 1))
====
