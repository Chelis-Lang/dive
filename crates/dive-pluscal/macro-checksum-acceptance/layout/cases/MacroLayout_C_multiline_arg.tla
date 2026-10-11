---- MODULE MacroLayout_C_multiline_arg ----
EXTENDS Integers,TLC
(* --algorithm A {variables x=0; macro Put(v,a){v:=a;}; {L:Put(x,1+
 2); E:skip;}} *)
====
