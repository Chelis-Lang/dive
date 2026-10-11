---- MODULE C_fair_with_member ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:with(a \in {0,1},b \in {1,2}){x:=a+b;} E:skip; } } *)
====
