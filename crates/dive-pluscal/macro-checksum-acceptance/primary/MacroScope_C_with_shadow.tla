---- MODULE MacroScope_C_with_shadow ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { with (a=3) { v:=a; } }; { L: Put(x,2); E: skip; } } *)
====
