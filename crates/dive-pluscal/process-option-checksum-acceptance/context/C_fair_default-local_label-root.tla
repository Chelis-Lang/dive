---- MODULE C_fair_default_local_label_root ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P=1) variable defaultInitValue; {L:g:=1; E:skip;}} *)
====
