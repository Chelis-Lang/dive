---- MODULE RecursiveHeader_C_unused_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0; procedure Recur(n=0) { R1:call Recur(n); R2:return; }; L:skip; } *)
====
