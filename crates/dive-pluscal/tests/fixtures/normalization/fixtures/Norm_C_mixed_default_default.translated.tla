---- MODULE Norm_C_mixed_default_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x,y=0; { L: x:=y; y:=1; E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "ac8e3132" /\ chksum(tla) = "bd95ee86")
CONSTANT defaultInitValue
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = defaultInitValue
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = y
     /\ y' = 1
     /\ pc' = "E"

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
