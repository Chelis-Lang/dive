---- MODULE C_corrected_parallel ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1) variables z=0; {L:g:=z+1 || z:=g+1; E:skip;}} *)
====
