---- MODULE Context_P_default_init ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x, y=0;
 define
 F(a) == a + 1
 end define;
begin L: x := F(1); end algorithm;
*)
====
