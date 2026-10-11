---- MODULE MacroExtra_P_auto_while ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin while v<a do v:=v+1; end while; end macro; begin Put(x, 1); skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f61a712" /\ chksum(tla) = "c9216393")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x < 1
               THEN /\ x' = x + 1
                    /\ pc' = "Lbl_1"
               ELSE /\ TRUE
                    /\ pc' = "Done"
                    /\ x' = x
         /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
