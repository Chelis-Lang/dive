---- MODULE Macro_P_if ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin if a>0 then v:=a; else v:=0; end if; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "abc6ab1" /\ chksum(tla) = "6a2a0e4b")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF 1 > 0
           THEN /\ x' = 1
           ELSE /\ x' = 0
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
