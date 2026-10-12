---- MODULE Define_P_comment ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == 1 \* comment
 G == F+1
 end define;
begin L: x := G; end algorithm;
*)
====
