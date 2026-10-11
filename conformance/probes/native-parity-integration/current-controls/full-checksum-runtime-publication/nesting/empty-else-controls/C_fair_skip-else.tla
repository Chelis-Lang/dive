---- MODULE C_fair_skip_else ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:if(x=0){x:=1;}else{skip;}; E:skip; } } *)
====
