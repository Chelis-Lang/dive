---- MODULE Macro_P_recursive ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin Put(v,a); end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
