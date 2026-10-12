---- MODULE Forms_P_function_undefined_bound ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in Missing]==a
 end define;
begin L:y:=0; E:skip;
end algorithm;
*)
====
