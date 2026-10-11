---- MODULE P_fair_junction_rhs ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0,y=0; begin L:x:=IF /\ TRUE
         /\ y=0 THEN 1 ELSE 2; E:skip; end algorithm; *)
====
