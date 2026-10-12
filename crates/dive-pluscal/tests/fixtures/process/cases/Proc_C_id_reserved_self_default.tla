---- MODULE Proc_C_id_reserved_self_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) { variables self=0;  L: self:=1; } } *)
====
