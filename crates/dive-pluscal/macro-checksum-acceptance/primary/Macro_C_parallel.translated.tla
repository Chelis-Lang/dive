---- MODULE Macro_C_parallel ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v:=a || y:=a+1; }; { L: Put(x, 1); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "a4af2064" /\ chksum(tla) = "faa8e273")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ /\ x' = 1
        /\ y' = 1+1
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
