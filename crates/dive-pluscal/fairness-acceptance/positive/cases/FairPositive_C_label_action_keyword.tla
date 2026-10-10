---- MODULE FairPositive_C_label_action_keyword ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P \in{1,2}) variables x=0; { WF: + x:=1; SF: - g:=x; } } *)
====
