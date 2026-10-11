---- MODULE C_terminal_empty_else ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:if(x=0){x:=1;}else{if(x=1){}else{};}; E:skip; } } *)
====
