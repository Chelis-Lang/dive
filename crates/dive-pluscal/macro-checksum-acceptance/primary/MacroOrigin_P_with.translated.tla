---- MODULE MacroOrigin_P_with ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin with t=a do v:=t; end with; end macro; begin Put(x, 1); skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5dfc1d70" /\ chksum(tla) = "3257a98f")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ LET t == 1 IN
              x' = t
         /\ TRUE
         /\ pc' = "Done"
         /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
