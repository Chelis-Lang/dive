---- MODULE MacroOrigin_P_with ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin with t=a do v:=t; end with; end macro; begin Put(x, 1); skip; end algorithm; *)
====
