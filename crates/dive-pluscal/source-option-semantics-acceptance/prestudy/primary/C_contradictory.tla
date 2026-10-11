---- MODULE C_contradictory ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf, -sf)
(* --algorithm A {variables g=0; process(P=1){g:=1;}} *)
====
