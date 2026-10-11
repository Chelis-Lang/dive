---- MODULE MacroLayout_P_unchanged_junction ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=TRUE /\ FALSE; end macro; begin L:Put(x,1); E:skip; end algorithm; *)
====
