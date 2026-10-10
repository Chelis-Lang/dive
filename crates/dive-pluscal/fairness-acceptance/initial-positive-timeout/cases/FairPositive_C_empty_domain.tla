---- MODULE FairPositive_C_empty_domain ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P \in{}) variables x=0; { L: + x:=1; E: - g:=x; } } *)
====
