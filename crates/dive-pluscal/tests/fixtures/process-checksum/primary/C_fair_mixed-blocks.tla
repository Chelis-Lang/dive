---- MODULE C_fair_mixed_blocks ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1){L:g:=g+1; E:skip;} process(Q \in {2,3}){variables z=0; M:z:=1; F:g:=z;}} *)
====
