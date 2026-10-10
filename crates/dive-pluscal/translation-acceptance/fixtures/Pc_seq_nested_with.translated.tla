---- MODULE Pc_seq_nested_with ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: with a=1,b \in {1,2} do x:=a+b; end with; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "fe3a73e8" /\ chksum(tla) = "f743e73e")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET a == 1 IN
          \E b \in {1,2}:
            x' = a+b
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
