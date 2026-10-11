---- MODULE C_plain_contradictory_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf, -sf)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
