---- MODULE FairInteraction_C_comment ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; fair process (P \in{1,(*domain comment*)2}) variable x=0; { L: + x:=1; E: - g:=x; } } *)
====
