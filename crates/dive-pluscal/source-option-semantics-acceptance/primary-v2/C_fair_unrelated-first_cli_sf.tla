---- MODULE C_fair_unrelated_first_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal ignored
\* PlusCal options (-wf)
(* --fair algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
