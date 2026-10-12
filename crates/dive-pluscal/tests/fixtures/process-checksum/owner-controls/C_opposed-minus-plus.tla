---- MODULE C_opposed_minus_plus ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; fair+ process(P = 1){L:-g:=1; E:skip;} fair+ process(Q = 2){M:+g:=2; F:skip;}} *)
====
