---- MODULE C_default_local ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0,defaultInitValue=9; process(P = 1)variables z; {L:g:=z; E:skip;}} *)
====
