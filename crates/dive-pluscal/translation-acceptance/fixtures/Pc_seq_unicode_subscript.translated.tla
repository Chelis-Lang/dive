---- MODULE Pc_seq_unicode_subscript ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable f=[a ↦ <<0,0>>]; begin L: f.a[1]:=1; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "b4f6331a" /\ chksum(tla) = "7ced05ed")
VARIABLES pc, f

vars == << pc, f >>

Init == (* Global variables *)
        /\ f = [a ↦ <<0,0>>]
        /\ pc = "L"

L == /\ pc = "L"
     /\ f' = [f EXCEPT !.a[1] = 1]
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ f' = f

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
