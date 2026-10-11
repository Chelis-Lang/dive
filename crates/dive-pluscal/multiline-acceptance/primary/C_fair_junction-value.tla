---- MODULE C_fair_junction_value ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:x:=IF /\ TRUE
  /\ y=0 THEN 1 ELSE 2; E:skip; } } *)
====
