---- MODULE Proc_C_id_invalid_goto_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) {  L: goto Unknown; } } *)
====
