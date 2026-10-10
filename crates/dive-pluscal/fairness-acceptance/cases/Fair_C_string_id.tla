---- MODULE Fair_C_string_id ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P ="worker") variables x=0; { L+: x:=1; E-: g:=x; } } *)
====
