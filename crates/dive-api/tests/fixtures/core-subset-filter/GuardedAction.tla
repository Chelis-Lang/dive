---- MODULE GuardedAction ----
EXTENDS Naturals
VARIABLES inbox, seen, done
Init == /\ inbox = {[seq |-> n, dest |-> [id |-> n % 20]] : n \in 1..40}
        /\ seen = {}
        /\ done = FALSE
Next == /\ ~done
        /\ \E selected \in SUBSET inbox :
              /\ \A m \in selected : m.dest.id = 1
              /\ \/ seen' = selected
                 \/ seen' = selected
        /\ done' = TRUE
        /\ UNCHANGED inbox
ReadsSelected ==
    /\ ~done
    /\ \E selected \in SUBSET inbox :
           /\ \A m \in selected : m.dest = selected
           /\ seen' = selected
    /\ done' = TRUE
    /\ UNCHANGED inbox
PartialInit == /\ inbox = {[other |-> 0], [id |-> 1]}
               /\ seen = {}
               /\ done = FALSE
PartialNext == /\ ~done
               /\ \E selected \in SUBSET inbox :
                      /\ \A m \in selected : m.id = 1
                      /\ seen' = selected
               /\ done' = TRUE
               /\ UNCHANGED inbox
NotDone == ~done
====
