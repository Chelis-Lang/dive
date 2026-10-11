---- MODULE Forms_P_local_constant_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL F==1
 end define;
process P \in {1,2} variable z=0; begin L:y:=F; E:skip; end process;
end algorithm;
*)
====
