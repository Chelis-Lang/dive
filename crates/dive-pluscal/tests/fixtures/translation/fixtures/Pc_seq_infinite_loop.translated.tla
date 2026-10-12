---- MODULE Pc_seq_infinite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: while TRUE do x := (x+1) % 2; end while; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "87f4e211" /\ chksum(tla) = "324173ad")
VARIABLE x

vars == << x >>

Init == (* Global variables *)
        /\ x = 0

Next == x' = (x+1) % 2

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
