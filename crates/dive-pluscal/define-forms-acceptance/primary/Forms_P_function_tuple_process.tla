---- MODULE Forms_P_function_tuple_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A
 variables x=0,y=0;
 define
 F[<<a,b>> \in {<<1,2>>}] == a+b
 end define;
process P \in {1,2} variable z=0; begin L:y:=F[<<1,2>>]; E:skip; end process;
end algorithm;
*)
====
