---- MODULE C_fair_branch_label ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1){L:if(g=0){M:g:=1;}else{g:=2;}; E:skip;}} *)
====
