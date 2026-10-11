---- MODULE C_fair_after_cli_default ----
EXTENDS Integers,TLC,Sequences

(* --fair algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
\* PlusCal options (-wf)
====
