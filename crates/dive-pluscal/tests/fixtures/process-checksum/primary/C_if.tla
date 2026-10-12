---- MODULE C_if ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1){L:if(g=0){g:=1;}else{g:=2;}; E:skip;}} *)
====
