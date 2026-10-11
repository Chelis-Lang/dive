---- MODULE C_weak_invalid_combination ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; fair process(P \in {1,2}){L:g:=1; E:skip;}} *)
====
