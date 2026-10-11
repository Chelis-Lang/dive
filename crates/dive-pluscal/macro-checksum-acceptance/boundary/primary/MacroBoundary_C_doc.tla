---- MODULE MacroBoundary_C_doc ----
EXTENDS Integers,TLC
(** --algorithm A { variables x=0; macro Put(v){ v:=1; }; { L:Put(x); E:skip; } }
**)
====
