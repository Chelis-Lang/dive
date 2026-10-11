---- MODULE MacroChecksum_P_source_label ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-label, -labelRoot M)
(* --algorithm A variables x=0, y=0; macro Put(v,a) begin v := a; end macro; begin Put(x,1); skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "12edf602" /\ chksum(tla) = "40484514")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "M1"

M1 == /\ pc = "M1"
      /\ x' = 1
      /\ TRUE
      /\ pc' = "Done"
      /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == M1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
