---- MODULE C_partial_invalid_combination ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P \in {1,2}){L:g:=1; g:=2;}} *)
====
