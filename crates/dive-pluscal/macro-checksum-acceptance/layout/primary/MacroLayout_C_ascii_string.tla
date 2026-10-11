---- MODULE MacroLayout_C_ascii_string ----
EXTENDS Integers,TLC
(* --algorithm A {variables x=0; macro Put(v,a){v:=a;}; {L:Put(x,"abc"); E:skip;}} *)
====
