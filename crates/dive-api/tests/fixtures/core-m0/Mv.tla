---- MODULE Mv ----
\* Model values in the core: untyped (p1, p2, Nil) and typed (t_a, t_b).
\* An untyped model value is unequal to every other value and in no set of
\* another kind; typed ones compare with their own type.
EXTENDS Integers, Sequences, FiniteSets
CONSTANTS Procs, Nil, Typed
VARIABLES owner, log
Init == /\ owner = [p \in Procs |-> Nil]
        /\ log = <<>>
Take(p) == /\ owner[p] = Nil
           /\ owner' = [owner EXCEPT ![p] = p]
           /\ log' = Append(log, p)
Drop(p) == /\ owner[p] = p
           /\ owner' = [owner EXCEPT ![p] = Nil]
           /\ UNCHANGED log
Next == /\ Len(log) < 3
        /\ \E p \in Procs : Take(p) \/ Drop(p)
Inv == /\ \A p \in Procs : owner[p] \in Procs \cup {Nil}
       /\ Nil \notin Procs
       /\ Nil # 1 /\ Nil # "Nil" /\ Nil # {Nil} /\ Nil # <<Nil>>
       /\ Nil \notin Nat /\ Nil \notin 1..3 /\ Nil \notin Seq(Procs)
       /\ (CHOOSE p \in Procs : TRUE) \in Procs
       /\ \A t \in Typed : t \in Typed /\ t # Nil
       /\ Cardinality(Typed) = 2
       /\ \A i \in 1..Len(log) : log[i] \in Procs
       /\ [r |-> Nil].r = Nil
       /\ {Nil, 1} \cup {Nil} = {1, Nil}
====
