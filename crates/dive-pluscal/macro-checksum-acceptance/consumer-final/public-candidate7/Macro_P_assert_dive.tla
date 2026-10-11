---- MODULE Macro_P_assert ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin assert a>0; v:=a; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "b94376c5" /\ chksum(tla) = "848b291f")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ Assert(1 > 0, "Failure of assertion at line 3, column 60 of macro called at line 3, column 98.")
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
