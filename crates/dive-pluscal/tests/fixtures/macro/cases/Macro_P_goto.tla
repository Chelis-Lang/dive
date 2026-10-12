---- MODULE Macro_P_goto ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin goto E; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
