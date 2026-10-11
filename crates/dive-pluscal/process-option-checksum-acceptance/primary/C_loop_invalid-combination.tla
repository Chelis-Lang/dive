---- MODULE C_loop_invalid_combination ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P \in {1,2}){while(g<2){g:=g+1;}}} *)
====
