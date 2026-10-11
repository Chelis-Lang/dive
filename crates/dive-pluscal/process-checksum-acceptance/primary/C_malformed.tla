---- MODULE C_malformed ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1){L:g:=; E:skip;}} *)
====
