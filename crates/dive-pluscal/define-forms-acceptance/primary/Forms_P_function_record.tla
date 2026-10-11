---- MODULE Forms_P_function_record ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}]==[value |-> a]
 end define;
begin L:y:=F[1].value; E:skip;
end algorithm;
*)
====
