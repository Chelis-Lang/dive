---- MODULE C_weak_over_strong ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf)
(* --algorithm A {variables g=0; process(P=1){g:=1;}} *)
====
