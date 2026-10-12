---- MODULE Pc_seq_c_current_infinite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: while (TRUE) {x:=(x+1)%2;} }}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "87f4e211" /\ chksum(tla) = "80068292")
VARIABLE x

vars == << x >>

Init == (* Global variables *)
        /\ x = 0

Next == x' = (x+1)%2

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
