---- MODULE P_fair_elsif_skip ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0,y=0; begin L:if x=0 then x:=1; elsif x=1 then x:=2; else skip; end if; E:skip; end algorithm; *)
====
