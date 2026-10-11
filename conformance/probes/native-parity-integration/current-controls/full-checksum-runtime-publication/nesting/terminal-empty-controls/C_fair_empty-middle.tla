---- MODULE C_fair_empty_middle ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:if(x=0){x:=1;}else{if(x=1){}else{if(x=2){x:=2;};};}; E:skip; } } *)
====
