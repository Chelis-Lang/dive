---- MODULE MacroExtra_P_triple ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro;macro Outer(v,a) begin Put(v,a+1); end macro;macro Last(v,a) begin Outer(v,a*2); end macro; begin L: Last(x, 2); E: skip; end algorithm; *)
====
