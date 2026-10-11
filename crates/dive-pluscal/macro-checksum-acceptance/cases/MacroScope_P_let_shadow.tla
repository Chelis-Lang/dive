---- MODULE MacroScope_P_let_shadow ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0,b=1; macro Put(v,a) begin v:=LET a == 3 IN a; end macro; begin L: Put(x,2); E: skip; end algorithm; *)
====
