---- MODULE Inline_P ----
EXTENDS Integers
(* --algorithm A variable x=0; macro Set(v,a) begin v:=a; end macro; begin L:Set(x,1+2); E:skip; end algorithm; *) Inv == x = 3
====
