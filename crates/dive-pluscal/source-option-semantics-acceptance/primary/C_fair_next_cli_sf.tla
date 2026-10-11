---- MODULE C_fair_next_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wfNext)
(* --fair algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
