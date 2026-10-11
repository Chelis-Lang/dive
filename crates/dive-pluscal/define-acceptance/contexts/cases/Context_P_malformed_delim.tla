---- MODULE Context_P_malformed_delim ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == (1]
 end define;
begin L: x := F(1); end algorithm;
*)
====
