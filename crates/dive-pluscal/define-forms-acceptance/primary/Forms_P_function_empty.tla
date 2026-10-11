---- MODULE Forms_P_function_empty ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {}]==a
 end define;
begin L:y:=Cardinality(DOMAIN F); E:skip;
end algorithm;
*)
====
