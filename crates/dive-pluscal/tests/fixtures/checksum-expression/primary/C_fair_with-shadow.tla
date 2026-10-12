---- MODULE C_fair_with_shadow ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:with(x=1,b=2){y:=x+b;} E:skip; } } *)
====
