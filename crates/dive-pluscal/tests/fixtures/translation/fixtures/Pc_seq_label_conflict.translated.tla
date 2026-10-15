---- MODULE Pc_seq_label_conflict ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable L=0; begin L: L := 1; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "8df1ccdf" /\ chksum(tla) = "4dec8bb1")
\* Label L at line 5 col 24 changed to L_
VARIABLES pc, L

vars == << pc, L >>

Init == (* Global variables *)
        /\ L = 0
        /\ pc = "L_"

L_ == /\ pc = "L_"
      /\ L' = 1
      /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ L' = L

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L_ \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
