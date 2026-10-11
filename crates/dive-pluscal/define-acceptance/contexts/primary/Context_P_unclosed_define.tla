---- MODULE Context_P_unclosed_define ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 
begin L: x := F(1); end algorithm;
*)
====
