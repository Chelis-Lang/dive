---- MODULE Macro_P_repeated ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro; begin L: Put(x,1); E: Put(y,2); F: skip; end algorithm; *)
====
