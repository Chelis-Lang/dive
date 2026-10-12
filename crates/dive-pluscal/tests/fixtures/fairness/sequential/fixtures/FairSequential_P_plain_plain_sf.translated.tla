---- MODULE FairSequential_P_plain_plain_sf ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0,x=0; begin L: x:=1; E: g:=x; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a072c028" /\ chksum(tla) = "bf5e46e7")
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
