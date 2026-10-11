---- MODULE C_fixed_with_nested ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:with(a=1,b=2){with(c=3,d=4){x:=a+b+c+d;}}; E:skip; } } *)
====
