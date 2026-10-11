---- MODULE C_plain_partial_unknown_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf, -unknown)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
