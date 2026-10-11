---- MODULE C_plain_word_suffix_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal optionsX (-wf)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
