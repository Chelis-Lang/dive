---- MODULE P_terminal_empty_else ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:if x=0 then x:=1; elsif x=1 then else end if; E:skip; end algorithm; *)
====
