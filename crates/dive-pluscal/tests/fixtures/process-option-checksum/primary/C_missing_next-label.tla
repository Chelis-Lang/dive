---- MODULE C_missing_next_label ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P \in {1,2}){g:=1; g:=2;}} *)
====
