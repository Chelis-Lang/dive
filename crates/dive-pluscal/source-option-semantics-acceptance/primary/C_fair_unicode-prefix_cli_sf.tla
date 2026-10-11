---- MODULE C_fair_unicode_prefix_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* λ PlusCal options (-wf)
(* --fair algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
