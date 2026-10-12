---- MODULE Proc_C_id_await_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) { variables x=0;  L: await (x=0); x:=1; E: skip; } } *)
====
