---- MODULE Define_P_recursive ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == IF a=0 THEN 0 ELSE F(a-1)
 end define;
begin L: x := F(2); end algorithm;
*)
====
