---- MODULE C_plain_root_only_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-labelRoot Src)
(* --algorithm A {variables g=0; process(P\in {1,2}){g:=1;}} *)
====
