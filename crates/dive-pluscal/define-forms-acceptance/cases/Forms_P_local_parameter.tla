---- MODULE Forms_P_local_parameter ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL F(a)==a+1
 end define;
begin L:y:=F(x); E:skip;
end algorithm;
*)
====
