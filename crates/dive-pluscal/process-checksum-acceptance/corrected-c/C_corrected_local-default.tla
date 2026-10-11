---- MODULE C_corrected_local_default ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1) variables z; {L:z:=1; E:g:=z;}} *)
====
