---- MODULE C_fair_two_constant ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1){L:g:=g+1; E:skip;} process(Q = 2){M:g:=g+1; F:skip;}} *)
====
