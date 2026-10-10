---- MODULE Fair_C_id_strong_option_wfNext ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair + process (P =1) variables x=0; { L+: x:=1; E-: g:=x; } } *)
====
