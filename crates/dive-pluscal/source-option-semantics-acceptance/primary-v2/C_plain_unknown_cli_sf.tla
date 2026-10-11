---- MODULE C_plain_unknown_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-unknown)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
