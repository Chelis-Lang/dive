---- MODULE Macro_C_assert ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { assert a>0; v:=a; }; { L: Put(x, 1); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "b94376c5" /\ chksum(tla) = "bd4ce1f8")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ Assert(1 > 0, "Failure of assertion at line 3, column 58 of macro called at line 3, column 84.")
     /\ x' = 1
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
