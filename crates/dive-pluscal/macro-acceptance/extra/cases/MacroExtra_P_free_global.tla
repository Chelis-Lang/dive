---- MODULE MacroExtra_P_free_global ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := y+a; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
