---- MODULE C_fair_token_suffix_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCalX options (-wf)
(* --fair algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
