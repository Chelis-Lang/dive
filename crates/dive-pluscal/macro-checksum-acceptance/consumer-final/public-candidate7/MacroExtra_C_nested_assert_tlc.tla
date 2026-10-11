---- MODULE MacroExtra_C_nested_assert ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { assert a>0; v := a; };macro Outer(v, a) { Put(v,a+1); }; { L: Outer(x,2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "3e45acb9" /\ chksum(tla) = "37e29108")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ Assert(( 2 + 1 ) > 0, "Failure of assertion at line 3, column 58 of macro called at line 3, column 120.")
     /\ x' = 2 + 1
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
