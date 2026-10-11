---- MODULE Macro_P_forward_call ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Outer(v, a) begin Put(v,a); end macro;macro Put(v, a) begin v := a; end macro; begin L: Outer(x,1); E: skip; end algorithm; *)
====
