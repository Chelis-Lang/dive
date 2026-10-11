---- MODULE MacroChecksum_P_source_weak ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf)
(* --algorithm A variables x=0, y=0; macro Put(v,a) begin v := a; end macro; begin L: Put(x,1); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "d06ecb2b" /\ chksum(tla) = "aac3bea2")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
