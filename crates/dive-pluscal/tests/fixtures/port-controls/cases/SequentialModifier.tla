---- MODULE SequentialModifier ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A { variable x=0; procedure Add(a=0) { Entry: + x:=x+a; Finish:return; }; { Start:call Add(1); Last:skip; } }
*)
====
