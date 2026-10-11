---- MODULE C_fair_underscore_suffix_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal_ options (-wf)
(* --fair algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
