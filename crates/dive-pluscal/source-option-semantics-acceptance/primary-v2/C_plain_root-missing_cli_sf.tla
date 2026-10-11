---- MODULE C_plain_root_missing_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-labelRoot)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
