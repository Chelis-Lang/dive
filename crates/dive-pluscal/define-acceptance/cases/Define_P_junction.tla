---- MODULE Define_P_junction ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == /\ x=0
     /\ y=0
 end define;
begin L: if F then x := 1; end if; end algorithm;
*)
====
