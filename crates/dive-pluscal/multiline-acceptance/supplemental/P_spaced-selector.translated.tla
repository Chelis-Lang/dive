---- MODULE P_spaced_selector ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x = <<0,0>>,y=0; begin L:x[1]:=1 +
 2; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "52c065b2" /\ chksum(tla) = "1354d8b")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = <<0,0>>
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [x EXCEPT ![1] =                                                          1 +
                              2]
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
