---- MODULE MacroChecksum_C_with ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { with (k = a) { v := k; } }; { L: Put(x, 1); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "61a3c7e7" /\ chksum(tla) = "bdf014ef")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET k == 1 IN
          x' = k
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
