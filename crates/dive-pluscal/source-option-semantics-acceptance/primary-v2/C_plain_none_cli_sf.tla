---- MODULE C_plain_none_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-nof)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
