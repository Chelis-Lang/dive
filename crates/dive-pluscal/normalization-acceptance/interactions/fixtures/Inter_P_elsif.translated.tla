---- MODULE Inter_P_elsif ----
EXTENDS Naturals,TLC
(* --algorithm A variables x=0,y=0; begin if x=0 then x:=1; elsif x=1 then y:=1; else skip; end if; x:=2; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "6a3e2fb2" /\ chksum(tla) = "865be933")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x=0
               THEN /\ x' = 1
                    /\ y' = y
               ELSE /\ IF x=1
                          THEN /\ y' = 1
                          ELSE /\ TRUE
                               /\ y' = y
                    /\ x' = x
         /\ pc' = "Lbl_2"

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = 2
         /\ pc' = "Done"
         /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
