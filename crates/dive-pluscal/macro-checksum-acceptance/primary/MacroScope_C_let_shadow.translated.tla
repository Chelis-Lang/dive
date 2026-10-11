---- MODULE MacroScope_C_let_shadow ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { v:=LET a == 3 IN a; }; { L: Put(x,2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "5a26b7b4" /\ chksum(tla) = "9e7b623")
VARIABLES pc, x, b

vars == << pc, x, b >>

Init == (* Global variables *)
        /\ x = 0
        /\ b = 1
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (LET 2 == 3 IN 2)
     /\ pc' = "E"
     /\ b' = b

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
