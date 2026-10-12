---- MODULE C_malformed ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=1 +
 ; E:skip; } } *)
====
