---- MODULE Macro_P_duplicate_macro ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro;macro Put(v, a) begin v:=a+1; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
