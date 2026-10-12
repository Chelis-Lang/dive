---- MODULE P_with_shadow ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:with x=1,b=2 do y:=x+b; end with; E:skip; end algorithm; *)
====
