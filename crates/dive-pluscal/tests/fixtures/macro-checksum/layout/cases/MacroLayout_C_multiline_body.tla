---- MODULE MacroLayout_C_multiline_body ----
EXTENDS Integers,TLC
(* --algorithm A {variables x=0; macro Put(v,a){v:=IF TRUE THEN 1
 ELSE 2;}; {L:Put(x,1); E:skip;}} *)
====
