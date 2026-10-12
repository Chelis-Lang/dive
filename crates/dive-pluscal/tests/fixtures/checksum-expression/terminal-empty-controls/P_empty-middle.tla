---- MODULE P_empty_middle ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:if x=0 then x:=1; elsif x=1 then elsif x=2 then x:=2; end if; E:skip; end algorithm; *)
====
