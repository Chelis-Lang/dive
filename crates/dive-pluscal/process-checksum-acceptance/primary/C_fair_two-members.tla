---- MODULE C_fair_two_members ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P \in {1,2}){L:g:=g+1; E:skip;} fair process(Q \in {3,4}){M:g:=g+1; F:skip;}} *)
====
