---- MODULE MacroExtra_P_with_rename ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin with t=a do v:=t; end with; end macro; begin L: Put(x, t); E: skip; end algorithm; *)
====
