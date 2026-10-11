---- MODULE C_fair_empty_either ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:either{}or{x:=1;}; E:skip; } } *)
====
