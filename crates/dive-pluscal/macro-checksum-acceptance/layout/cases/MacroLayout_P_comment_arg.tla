---- MODULE MacroLayout_P_comment_arg ----
EXTENDS Integers,TLC
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L:Put(x,1 (* inline *) + 2); E:skip; end algorithm; *)
====
