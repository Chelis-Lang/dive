---- MODULE Macro_P_either ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin either v:=a; or v:=0; end either; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
