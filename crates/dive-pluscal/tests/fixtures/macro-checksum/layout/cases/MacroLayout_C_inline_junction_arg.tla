---- MODULE MacroLayout_C_inline_junction_arg ----
EXTENDS Integers,TLC
(* --algorithm A {variables x=0; macro Put(v,a){v:=a;}; {L:Put(x,TRUE /\ FALSE); E:skip;}} *)
====
