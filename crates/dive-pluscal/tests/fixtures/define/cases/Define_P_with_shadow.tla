---- MODULE Define_P_with_shadow ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a+1
 end define;
begin L: with F=2 do x := F; end with; end algorithm;
*)
====
