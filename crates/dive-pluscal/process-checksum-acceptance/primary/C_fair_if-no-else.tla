---- MODULE C_fair_if_no_else ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1){L:if(g=0){g:=1;}; E:skip;}} *)
====
