---- MODULE C_tight_open ----
EXTENDS Integers,TLC,Sequences
(*PlusCal options (-wf)*)
(* --algorithm A {variables g=0; process(P=1){L:g:=1; E:skip;}} *)
====
