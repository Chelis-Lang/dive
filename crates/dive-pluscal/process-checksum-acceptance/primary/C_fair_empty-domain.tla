---- MODULE C_fair_empty_domain ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P \in {}){L:g:=g+1; E:skip;}} *)
====
