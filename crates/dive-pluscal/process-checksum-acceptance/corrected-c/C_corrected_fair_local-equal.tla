---- MODULE C_corrected_fair_local_equal ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1) variables z=0; {L:z:=z+1; E:g:=z;}} *)
====
