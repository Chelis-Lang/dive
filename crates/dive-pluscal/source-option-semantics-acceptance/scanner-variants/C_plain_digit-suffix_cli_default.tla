---- MODULE C_plain_digit_suffix_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal9 options (-wf)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
