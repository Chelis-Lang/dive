---- MODULE C_plain_token_suffix_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCalX options (-wf)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
