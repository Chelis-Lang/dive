---- MODULE C_fair_with ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1){L:with(a\in {1,2}){g:=a;}; E:skip;}} *)
====
