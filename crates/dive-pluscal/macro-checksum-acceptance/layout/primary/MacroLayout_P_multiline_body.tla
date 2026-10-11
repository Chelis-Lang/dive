---- MODULE MacroLayout_P_multiline_body ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=IF TRUE THEN 1
 ELSE 2; end macro; begin L:Put(x,1); E:skip; end algorithm; *)
====
