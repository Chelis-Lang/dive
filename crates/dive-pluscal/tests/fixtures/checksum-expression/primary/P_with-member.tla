---- MODULE P_with_member ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:with a \in {0,1},b \in {1,2} do x:=a+b; end with; E:skip; end algorithm; *)
====
