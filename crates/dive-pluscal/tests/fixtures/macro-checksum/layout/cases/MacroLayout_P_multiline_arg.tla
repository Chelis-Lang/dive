---- MODULE MacroLayout_P_multiline_arg ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L:Put(x,1+
 2); E:skip; end algorithm; *)
====
