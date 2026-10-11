---- MODULE Forms_C_function_constant_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}] == a+1
 }
process (P \in {1,2}) variable z=0; { L:y:=F[1]; E:skip; }
}
*)
====
