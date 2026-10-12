---- MODULE Proc_C_set_invalid_goto_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) {  L: goto Unknown; } } *)
====
