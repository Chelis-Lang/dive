---- MODULE RecursiveHeader_C_zero_param_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur() { R1:call Recur(); return; }; L:call Recur(); E:skip; } *)
====
