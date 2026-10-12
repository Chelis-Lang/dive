---- MODULE C_default_local_weak ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P=1) variable defaultInitValue; {L:g:=1; E:skip;}} *)
====
