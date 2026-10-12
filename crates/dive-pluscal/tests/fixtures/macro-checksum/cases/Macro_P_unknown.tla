---- MODULE Macro_P_unknown ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro; begin L: Nope(x,1); E: skip; end algorithm; *)
====
