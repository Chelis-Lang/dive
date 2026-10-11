---- MODULE ClosingLine_P_block_comment ----
EXTENDS Integers
(* --algorithm A variable x=0; macro Set(v,a) begin v:=a; end macro; begin L:Set(x,1+2); E:skip; end algorithm; *) (* note *)
Inv == x = 3
====
