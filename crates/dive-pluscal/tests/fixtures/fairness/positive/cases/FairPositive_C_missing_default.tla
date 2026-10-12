---- MODULE FairPositive_C_missing_default ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P \in{1,2}) variables x=0; { x:=1; E: + g:=x; } } *)
====
