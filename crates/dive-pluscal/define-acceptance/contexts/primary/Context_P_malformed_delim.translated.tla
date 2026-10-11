---- MODULE Context_P_malformed_delim ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == (1]
 end define;
begin L: x := F(1); end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "795da128" /\ chksum(tla) = "2642346b")
VARIABLES pc, x, y

(* define statement *)
F(a) == (1]


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F(1)
     /\ pc' = "Done"
     /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
