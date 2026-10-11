---- MODULE MacroScope_C_let_shadow ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { v:=LET a == 3 IN a; }; { L: Put(x,2); E: skip; } } *)
====
