---- MODULE C_empty_else ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:if(x=0){x:=1+
  2;}else{}; E:skip; } } *)
====
