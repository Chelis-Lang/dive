---- MODULE C_plain_root_last_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-labelRoot One, -labelRoot Two, -label)
(* --algorithm A {variables g=0; process(P\in {1,2}){g:=1;}} *)
====
