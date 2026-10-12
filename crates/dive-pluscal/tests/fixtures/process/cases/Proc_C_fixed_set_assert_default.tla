---- MODULE Proc_C_fixed_set_assert_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: x:=1; assert x=0; E: skip; } } *)
====
