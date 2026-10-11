---- MODULE Forms_P_local_parameter_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 LOCAL F(a)==a+1
 end define;
process P \in {1,2} variable z=0; begin L:y:=F(x); E:skip; end process;
end algorithm;
*)
====
