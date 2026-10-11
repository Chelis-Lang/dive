---- MODULE Define_P_multiline ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) ==
  IF a > 0 THEN a ELSE 0
 end define;
begin L: x := F(1); end algorithm;
*)
====
