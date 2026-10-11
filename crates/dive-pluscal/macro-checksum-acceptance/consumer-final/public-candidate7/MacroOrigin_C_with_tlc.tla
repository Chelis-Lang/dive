---- MODULE MacroOrigin_C_with ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { with (t = a) { v:=t; } }; { Put(x, 1); skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "5dfc1d70" /\ chksum(tla) = "3713e3eb")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ LET t == 1 IN
               x' = t
         /\ TRUE
         /\ pc' = "Done"
         /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
