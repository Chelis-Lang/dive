---- MODULE C_fair_parallel ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A { variables x=0,y=1; { L: x:=y || y:=x; } } *)
====
