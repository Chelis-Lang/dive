---- MODULE C_plain_strong ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P \in {1,2}){L:g:=1; E:skip;}} *)
====
