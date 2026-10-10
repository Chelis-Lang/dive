---- MODULE Proc_C_id_infinite_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) { variables x=0;  L: while (TRUE) { x:=(x+1)%2; }; } } *)
====
