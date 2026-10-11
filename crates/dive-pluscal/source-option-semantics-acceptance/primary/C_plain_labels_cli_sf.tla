---- MODULE C_plain_labels_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-label, -labelRoot Src)
(* --algorithm A {variables g=0; process(P\in {1,2}){g:=1;}} *)
====
