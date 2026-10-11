---- MODULE C_with_three ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:with(a=1,b=2,c=3){x:=a+b+c;} E:skip; } } *)
====
