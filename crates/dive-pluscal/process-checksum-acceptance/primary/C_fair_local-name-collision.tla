---- MODULE C_fair_local_name_collision ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1){variables z=0; L:g:=g+1; E:skip;} process(Q = 2){variables z=0; M:z:=1; F:skip;}} *)
====
