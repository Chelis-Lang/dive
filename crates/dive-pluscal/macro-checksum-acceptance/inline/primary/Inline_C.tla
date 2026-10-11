---- MODULE Inline_C ----
EXTENDS Integers
(* --algorithm A { variable x=0; macro Set(v,a) { v:=a; }; { L:Set(x,1+2); E:skip; } } *) Inv == x = 3
====
