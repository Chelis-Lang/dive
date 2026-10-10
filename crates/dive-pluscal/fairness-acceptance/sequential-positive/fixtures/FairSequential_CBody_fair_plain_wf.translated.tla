---- MODULE FairSequential_CBody_fair_plain_wf ----
EXTENDS Naturals,TLC
(* --fair algorithm A { variable g=0,x=0; { L: x:=1; E: g:=x; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "bf5e46e7")
VARIABLES pc, g, x

vars == << pc, g, x >>

Init == (* Global variables *)
        /\ g = 0
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "E"
     /\ g' = g

E == /\ pc = "E"
     /\ g' = x
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
