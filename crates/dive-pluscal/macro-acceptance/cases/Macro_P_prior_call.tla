---- MODULE Macro_P_prior_call ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro;macro Outer(v, a) begin Put(v,a+1); end macro; begin L: Outer(x,2); E: skip; end algorithm; *)
====
