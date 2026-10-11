---- MODULE C_plain_tabs_cli_default ----
EXTENDS Integers,TLC,Sequences
\*	PlusCal	options	(	-wf	)
(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
====
