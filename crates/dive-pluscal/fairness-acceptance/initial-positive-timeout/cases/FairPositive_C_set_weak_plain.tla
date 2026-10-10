---- MODULE FairPositive_C_set_weak_plain ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P \in{1,2}) variables x=0; { L: x:=1; E: g:=x; } } *)
====
