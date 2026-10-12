---- MODULE MacroLayout_C_unicode_comment ----
EXTENDS Integers,TLC
\* λ😀 outside quoted tokens
(* --algorithm A {variables x=0; macro Put(v,a){v:=a;}; {L:Put(x,1); E:skip;}} *)
====
