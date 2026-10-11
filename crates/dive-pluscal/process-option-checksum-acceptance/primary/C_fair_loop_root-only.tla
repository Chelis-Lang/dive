---- MODULE C_fair_loop_root_only ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P \in {1,2}){while(g<2){g:=g+1;}}} *)
====
