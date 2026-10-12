---- MODULE C_default_name ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0,defaultInitValue=9; process(P = 1){L:g:=1; E:skip;}} *)
====
