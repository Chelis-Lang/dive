---- MODULE MacroLayout_P_inline_junction_arg ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L:Put(x,TRUE /\ FALSE); E:skip; end algorithm; *)
====
