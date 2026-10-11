---- MODULE C_fair_with_dependent ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:with(a=1,b=a+1){x:=a+b;} E:skip; } } *)
====
