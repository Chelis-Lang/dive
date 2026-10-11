---- MODULE C_none_over_weak ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-nof)
(* --algorithm A {variables g=0; process(P=1){L:g:=1; E:skip;}} *)
====
