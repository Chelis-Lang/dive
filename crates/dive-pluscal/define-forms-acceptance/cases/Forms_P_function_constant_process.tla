---- MODULE Forms_P_function_constant_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[a \in {1,2}] == a+1
 end define;
process P \in {1,2} variable z=0; begin L:y:=F[1]; E:skip; end process;
end algorithm;
*)
====
