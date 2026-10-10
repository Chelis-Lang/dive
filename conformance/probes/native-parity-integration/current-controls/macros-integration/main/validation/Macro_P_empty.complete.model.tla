---- MODULE Macro_P_empty ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put() begin  end macro; begin L: Put(); E: skip; end algorithm; *)

VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
