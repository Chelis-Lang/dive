---- MODULE MacroLayout_P_unicode_comment ----
EXTENDS Integers,TLC
\* λ😀 outside quoted tokens
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L:Put(x,1); E:skip; end algorithm; *)
====
