---- MODULE Define_C_junction ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == /\ x=0
     /\ y=0
 }
{ L: if (F) { x := 1; }; }
}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "321782c3" /\ chksum(tla) = "f186803c")
VARIABLES pc, x, y

(* define statement *)
F == /\ x=0
    /\ y=0

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF F
           THEN /\ x' = 1
           ELSE /\ TRUE
                /\ x' = x
     /\ pc' = "Done"
     /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
