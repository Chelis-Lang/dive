---- MODULE Proc_C_fixed_id_infinite_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=0;  { L: while (TRUE) { x:=(x+1)%2; }; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "9820a89e" /\ chksum(tla) = "320fd72a")
VARIABLES g, x

vars == << g, x >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0

P == /\ x' = (x+1)%2
     /\ g' = g

Next == P

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
