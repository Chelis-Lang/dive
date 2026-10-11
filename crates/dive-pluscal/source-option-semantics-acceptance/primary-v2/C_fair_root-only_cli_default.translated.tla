---- MODULE C_fair_root_only_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-labelRoot Src)
(* --fair algorithm A {variables g=0; process(P\in {1,2}){g:=1;}} *)
====
