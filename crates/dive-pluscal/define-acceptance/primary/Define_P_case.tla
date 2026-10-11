---- MODULE Define_P_case ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == CASE a=1 -> 2 [] OTHER -> 0
 end define;
begin L: x := F(1); end algorithm;
*)
====
