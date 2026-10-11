---- MODULE C_fair_self ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P \in {1,2}){variables z=self; L:g:=z; E:skip;}} *)
====
