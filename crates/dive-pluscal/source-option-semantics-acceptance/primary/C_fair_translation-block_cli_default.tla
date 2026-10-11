---- MODULE C_fair_translation_block_cli_default ----
EXTENDS Integers,TLC,Sequences
\* BEGIN TRANSLATION
\* PlusCal options (-wf)
\* END TRANSLATION
(* --fair algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
