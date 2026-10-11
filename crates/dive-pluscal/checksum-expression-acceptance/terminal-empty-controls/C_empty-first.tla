---- MODULE C_empty_first ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:if(x=0){}else{if(x=1){x:=1;};}; E:skip; } } *)
====
