---- MODULE FairInteraction_P_wrapped ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; fair process P \in{1,2} variable x=0; begin VeryLongFirstFairnessActionLabel: + x:=1; VeryLongLastFairnessActionLabel: - g:=x; end process; end algorithm; *)
====
