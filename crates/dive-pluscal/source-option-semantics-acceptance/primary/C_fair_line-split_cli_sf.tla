---- MODULE C_fair_line_split_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal
\* options (-wf)
(* --fair algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
