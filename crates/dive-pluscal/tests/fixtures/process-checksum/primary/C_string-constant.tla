---- MODULE C_string_constant ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = "p"){L:g:=g+1; E:skip;}} *)
====
