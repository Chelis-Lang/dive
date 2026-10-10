---- MODULE FairCombined_C_id_weak_option_sf ----
EXTENDS Naturals,TLC
(* --fair algorithm A { variables g=0; fair process (P =1) variables x=0; { L: + x:=1; E: - g:=x; } } *)
====
