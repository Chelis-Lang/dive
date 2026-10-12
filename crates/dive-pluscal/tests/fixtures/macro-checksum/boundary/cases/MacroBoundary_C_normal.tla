---- MODULE MacroBoundary_C_normal ----
EXTENDS Integers,TLC
(* --algorithm A { variables x=0; macro Put(v){ v:=1; }; { L:Put(x); E:skip; } }
*)
====
