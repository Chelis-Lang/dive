---- MODULE C_plain_after_cli_sf ----
EXTENDS Integers,TLC,Sequences

(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
\* PlusCal options (-wf)
====
