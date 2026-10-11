---- MODULE C_plain_alias_next_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (wfNext)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
