---- MODULE Forms_P_local_constant ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL F==1
 end define;
begin L:y:=F; E:skip;
end algorithm;
*)
====
