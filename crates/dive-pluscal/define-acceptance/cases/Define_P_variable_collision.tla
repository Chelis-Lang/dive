---- MODULE Define_P_variable_collision ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 x == 1
 end define;
begin L: x := 2; end algorithm;
*)
====
