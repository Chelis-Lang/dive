---- MODULE Forms_P_function_helper_shadow ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL F==1 G[F \in {1,2}]==F
 end define;
begin L:y:=G[1]; E:skip;
end algorithm;
*)
====
