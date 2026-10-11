---- MODULE P_fair_shape_nested_with ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0; begin L: with a=1 do with b=2 do x:=a+b; end with; end with; end algorithm; *)
====
