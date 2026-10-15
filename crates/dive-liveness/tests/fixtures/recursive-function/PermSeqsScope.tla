---- MODULE PermSeqsScope ----
EXTENDS Naturals, Sequences, FiniteSets
CONSTANT N
VARIABLE pending, done
vars == <<pending, done>>
PermSeqs(S) ==
  LET perms[ss \in SUBSET S] ==
       IF ss = {} THEN { << >> }
       ELSE LET ps == [ x \in ss |->
                        { Append(sq,x) : sq \in perms[ss \ {x}] } ]
            IN UNION { ps[x] : x \in ss }
  IN perms[S]
Init == pending = 1..N /\ done = FALSE
Schedule == \E sq \in PermSeqs(pending) :
              /\ Len(sq) = Cardinality(pending)
              /\ pending' = {}
              /\ done' = TRUE
Next == Schedule
Spec == Init /\ [][Next]_vars /\ WF_vars(Schedule)
Good == <>done
Bad == []<>(~done)
====
