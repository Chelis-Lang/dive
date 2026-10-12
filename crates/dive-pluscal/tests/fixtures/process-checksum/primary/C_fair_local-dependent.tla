---- MODULE C_fair_local_dependent ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P \in {1,2}){variables z=0,w\in {z}; L:g:=w; E:skip;}} *)
====
