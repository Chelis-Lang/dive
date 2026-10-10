---- MODULE FairSequential_CBody_fair_modifiers_wf ----
EXTENDS Naturals,TLC
(* --fair algorithm A { variable g=0,x=0; { L: + x:=1; E: - g:=x; } } *)
====
