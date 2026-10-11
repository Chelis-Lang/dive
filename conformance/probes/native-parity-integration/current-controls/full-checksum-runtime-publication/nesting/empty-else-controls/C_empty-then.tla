---- MODULE C_empty_then ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:if(x=0){}else{x:=1;}; E:skip; } } *)
====
