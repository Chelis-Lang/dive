---- MODULE FairCombined_C_set_unfair_option_wf ----
EXTENDS Naturals,TLC
(* --fair algorithm A { variables g=0;  process (P \in{1,2}) variables x=0; { L: + x:=1; E: - g:=x; } } *)
====
