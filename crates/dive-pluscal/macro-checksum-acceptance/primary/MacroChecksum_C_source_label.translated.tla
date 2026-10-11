---- MODULE MacroChecksum_C_source_label ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-label, -labelRoot M)
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { Put(x,1); skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "12edf602" /\ chksum(tla) = "9b6c5bfe")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "M1"

M1 == /\ pc = "M1"
      /\ x' = 1
      /\ TRUE
      /\ pc' = "Done"
      /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == M1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
