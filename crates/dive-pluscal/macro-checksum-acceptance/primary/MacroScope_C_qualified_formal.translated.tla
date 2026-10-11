---- MODULE MacroScope_C_qualified_formal ----
EXTENDS Integers, TLC
---- MODULE Helper ----
a == 2
====
I == INSTANCE Helper
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { v:=I!a; }; { L: Put(x,2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "c43a8184" /\ chksum(tla) = "b91b6eb")
VARIABLES pc, x, b

vars == << pc, x, b >>

Init == (* Global variables *)
        /\ x = 0
        /\ b = 1
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = I!2
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
