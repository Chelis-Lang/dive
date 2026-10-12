---- MODULE C_missing_none ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P \in {1,2}){g:=1; g:=2;}} *)
====
