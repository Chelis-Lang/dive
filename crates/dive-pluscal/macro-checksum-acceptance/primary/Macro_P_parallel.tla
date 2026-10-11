---- MODULE Macro_P_parallel ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v:=a || y:=a+1; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
