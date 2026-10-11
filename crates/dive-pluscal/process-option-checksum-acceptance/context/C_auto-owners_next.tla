---- MODULE C_auto_owners_next ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P=1){g:=1; g:=2;} process(Q \in {2,3}){g:=1; g:=2;}} *)
====
