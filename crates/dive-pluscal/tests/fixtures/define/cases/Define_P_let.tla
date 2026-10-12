---- MODULE Define_P_let ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == LET G == a+1 IN G
 end define;
begin L: x := F(2); end algorithm;
*)
====
