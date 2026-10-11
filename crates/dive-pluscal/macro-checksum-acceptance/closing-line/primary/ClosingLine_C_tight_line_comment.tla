---- MODULE ClosingLine_C_tight_line_comment ----
EXTENDS Integers
(* --algorithm A { variable x=0; macro Set(v,a) { v:=a; }; { L:Set(x,1+2); E:skip; } } *)\* note
Inv == x = 3
====
