---- MODULE C_fair_label_collision ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1){L:g:=g+1; E:skip;} process(Q = 2){L:g:=1; E:skip;}} *)
====
