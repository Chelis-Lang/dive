---- MODULE Define_P_sequence ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == <<a,a>>
 end define;
begin L: x := F(2); end algorithm;
*)
====
