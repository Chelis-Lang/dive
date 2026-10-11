---- MODULE MacroLayout_C_body_junction ----
EXTENDS Integers,TLC
(* --algorithm A {variables x=0; macro Put(v,a){v:=a /\ TRUE;}; {L:Put(x,TRUE); E:skip;}} *)
====
