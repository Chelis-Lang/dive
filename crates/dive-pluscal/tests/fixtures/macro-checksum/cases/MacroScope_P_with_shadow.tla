---- MODULE MacroScope_P_with_shadow ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0,b=1; macro Put(v,a) begin with a=3 do v:=a; end with; end macro; begin L: Put(x,2); E: skip; end algorithm; *)
====
