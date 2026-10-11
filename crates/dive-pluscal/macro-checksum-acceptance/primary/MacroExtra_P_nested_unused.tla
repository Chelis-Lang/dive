---- MODULE MacroExtra_P_nested_unused ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro;macro Outer(v,a) begin Put(v,a); end macro; begin L: x:=1; E: skip; end algorithm; *)
====
