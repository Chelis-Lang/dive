---- MODULE MacroLayout_C_unchanged_junction ----
EXTENDS Integers,TLC
(* --algorithm A {variables x=0; macro Put(v,a){v:=TRUE /\ FALSE;}; {L:Put(x,1); E:skip;}} *)
====
