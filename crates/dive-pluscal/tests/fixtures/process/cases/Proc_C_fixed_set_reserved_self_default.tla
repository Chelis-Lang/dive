---- MODULE Proc_C_fixed_set_reserved_self_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables self=0;  { L: self:=1; } } *)
====
