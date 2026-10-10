---- MODULE Pc_seq_c_current_fields ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable f=[a|->0,b|->0]; { L: f.a:=1 || f.b:=2; E: skip; }}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "27a7e975" /\ chksum(tla) = "705b7638")
VARIABLES pc, f

vars == << pc, f >>

Init == (* Global variables *)
        /\ f = [a|->0,b|->0]
        /\ pc = "L"

L == /\ pc = "L"
     /\ f' = [f EXCEPT !.a = 1,
                       !.b = 2]
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
