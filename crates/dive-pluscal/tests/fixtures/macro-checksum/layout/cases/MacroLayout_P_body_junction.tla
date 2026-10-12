---- MODULE MacroLayout_P_body_junction ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a /\ TRUE; end macro; begin L:Put(x,TRUE); E:skip; end algorithm; *)
====
