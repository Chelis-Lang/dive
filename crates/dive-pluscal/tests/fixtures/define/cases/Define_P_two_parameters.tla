---- MODULE Define_P_two_parameters ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a,b) == a+b
 end define;
begin L: x := F(1,2); end algorithm;
*)
====
