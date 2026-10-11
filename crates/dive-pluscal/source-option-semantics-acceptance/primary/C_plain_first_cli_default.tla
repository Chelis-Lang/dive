---- MODULE C_plain_first_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf)
\* PlusCal options (-sf)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
