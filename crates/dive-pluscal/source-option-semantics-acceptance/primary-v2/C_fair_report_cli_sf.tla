---- MODULE C_fair_report_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-reportLabels)
(* --fair algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
