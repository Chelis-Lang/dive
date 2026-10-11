---- MODULE Forms_P_local_chain ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL G(a)==a+1 LOCAL F(a)==G(a)
 end define;
begin L:y:=F(x); E:skip;
end algorithm;
*)
====
