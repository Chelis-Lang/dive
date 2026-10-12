---- MODULE C_with ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1){L:with(a\in {1,2}){g:=a;}; E:skip;}} *)
====
