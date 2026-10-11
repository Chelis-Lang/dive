---- MODULE MacroOrigin_C_if ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { if (a>0) { v:=a; } else { v:=0; }; }; macro Outer(v,a) { Put(v,a); }; { Outer(x, 1); skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "54e5f807" /\ chksum(tla) = "c790429c")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF 1 > 0
               THEN /\ x' = 1
               ELSE /\ x' = 0
         /\ TRUE
         /\ pc' = "Done"
         /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
