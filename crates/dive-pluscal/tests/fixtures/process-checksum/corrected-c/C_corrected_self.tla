---- MODULE C_corrected_self ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P \in {1,2}) variables z=self; {L:g:=z; E:skip;}} *)
====
