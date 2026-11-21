---- MODULE TunableSetHistory ----
\* Executable reconstruction of the stamped FIFO operations in Rijo (2018), Algorithms 5.3–5.4.
\* The state queries and history-replay policies from Appendix E are separate properties.
EXTENDS Naturals, Sequences, FiniteSets, TLC
CONSTANTS Replicas, Elements, OpsPerReplica, Policies
VARIABLES clock, issued, known, history, inbox, head, pc
vars == <<clock, issued, known, history, inbox, head, pc>>
MaxIssued == Cardinality(Replicas) * OpsPerReplica
Init == /\ clock = 0 /\ issued = [r \in Replicas |-> 0]
        /\ known = [r \in Replicas |-> {}]
        /\ history = [r \in Replicas |-> <<>>] /\ inbox = history
        /\ head = [r \in Replicas |-> 0] /\ pc = [r \in Replicas |-> "While"]

Issue(r, e, op, p) ==
    LET x == [stamp |-> clock + 1, author |-> r, elem |-> e, kind |-> op,
              policy |-> p, observed |-> known[r]]
    IN /\ pc[r] = "While" /\ issued[r] < OpsPerReplica
       /\ Cardinality(known[r]) < MaxIssued
       /\ clock' = clock + 1 /\ issued' = [issued EXCEPT ![r] = @ + 1]
       /\ known' = [known EXCEPT ![r] = @ \cup {x}]
       /\ history' = [history EXCEPT ![r] = Append(@, x)]
       /\ inbox' = [i \in Replicas |-> IF i = r THEN inbox[i] ELSE Append(inbox[i], x)]
       /\ UNCHANGED <<head, pc>>

Receive(r) ==
    LET x == Head(inbox[r])
    IN /\ pc[r] = "While" /\ Cardinality(known[r]) < MaxIssued /\ Len(inbox[r]) > 0
       /\ history' = [history EXCEPT ![r] = Append(@, x)]
       /\ known' = [known EXCEPT ![r] = @ \cup {x}]
       /\ head' = [head EXCEPT ![r] = x]
       /\ inbox' = [inbox EXCEPT ![r] = Tail(@)]
       /\ UNCHANGED <<clock, issued, pc>>

EmptyPoll(r) == /\ pc[r] = "While" /\ issued[r] < OpsPerReplica /\ Len(inbox[r]) = 0
                /\ UNCHANGED vars
AtEnd(r) == /\ pc[r] = "While" /\ Cardinality(known[r]) = MaxIssued
            /\ pc' = [pc EXCEPT ![r] = "End"]
            /\ UNCHANGED <<clock, issued, known, history, inbox, head>>
Finish(r) == /\ pc[r] = "End" /\ pc' = [pc EXCEPT ![r] = "Done"]
             /\ UNCHANGED <<clock, issued, known, history, inbox, head>>
Next == \/ \E r \in Replicas :
             \/ \E e \in Elements, op \in {"add", "remove"}, p \in Policies : Issue(r, e, op, p)
             \/ Receive(r) \/ EmptyPoll(r) \/ AtEnd(r) \/ Finish(r)
        \/ /\ \A r \in Replicas : pc[r] = "Done"
           /\ UNCHANGED vars
HistoryAgreement == \A r \in Replicas :
    /\ Len(history[r]) = Cardinality(known[r])
    /\ known[r] = {history[r][i] : i \in 1..Len(history[r])}
    /\ Len(history[r]) + Len(inbox[r]) = clock
Convergence == (\A r \in Replicas : pc[r] = "Done") =>
               (\A a, b \in Replicas : known[a] = known[b])
Symmetry == Permutations(Replicas) \cup Permutations(Elements)
====
