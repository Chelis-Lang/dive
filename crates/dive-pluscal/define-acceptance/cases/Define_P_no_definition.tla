---- MODULE Define_P_no_definition ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 x + 1
 end define;
begin L: x := 1; end algorithm;
*)
====
