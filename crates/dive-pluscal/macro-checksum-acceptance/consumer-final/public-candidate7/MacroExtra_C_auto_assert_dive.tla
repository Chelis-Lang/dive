---- MODULE MacroExtra_C_auto_assert ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { assert a>0; v:=a; }; { Put(x, 1); skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "4d4f619b" /\ chksum(tla) = "26b4ab30")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ Assert(1 > 0, "Failure of assertion at line 3, column 58 of macro called at line 3, column 81.")
         /\ x' = 1
         /\ TRUE
         /\ pc' = "Done"
         /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
