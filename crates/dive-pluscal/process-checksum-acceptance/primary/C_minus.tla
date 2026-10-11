---- MODULE C_minus ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; fair+ process(P = 1){L-:g:=1; E:skip;}} *)
====
