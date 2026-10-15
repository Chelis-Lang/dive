---- MODULE EmptyThen ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; begin L:if x=0 then else x:=1; end if; E:skip; end algorithm;
*)
====
