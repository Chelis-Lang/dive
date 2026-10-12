---- MODULE Macro_P_with_capture ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin with a=2 do v:=a; end with; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
