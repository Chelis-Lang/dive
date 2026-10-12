---- MODULE Define_P_set ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == {a,1}
 end define;
begin L: x := F(2); end algorithm;
*)
====
