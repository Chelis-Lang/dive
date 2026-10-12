---- MODULE Proc_C_fixed_set_await_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: await (x=0); x:=1; E: skip; } } *)
====
