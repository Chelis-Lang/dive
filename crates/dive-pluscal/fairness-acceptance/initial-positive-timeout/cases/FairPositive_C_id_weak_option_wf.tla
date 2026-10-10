---- MODULE FairPositive_C_id_weak_option_wf ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P =1) variables x=0; { L: + x:=1; E: - g:=x; } } *)
====
