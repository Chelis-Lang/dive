---- MODULE C_while ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1){L:while(g<2){g:=g+1;}; E:skip;}} *)
====
