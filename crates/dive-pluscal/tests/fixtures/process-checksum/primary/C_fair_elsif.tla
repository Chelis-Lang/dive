---- MODULE C_fair_elsif ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1){L:if(g=0){g:=1;}else if(g=1){g:=2;}else{g:=3;}; E:skip;}} *)
====
