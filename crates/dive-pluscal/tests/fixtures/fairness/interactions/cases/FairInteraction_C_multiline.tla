---- MODULE FairInteraction_C_multiline ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; fair process (P \in{1,
2}) variable x=0; { L: + x:=1; E: - g:=x; } } *)
====
