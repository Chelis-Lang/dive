---- MODULE Context_P_label_flag ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 end define;
begin L: x := F(1); end algorithm;
*)
====
