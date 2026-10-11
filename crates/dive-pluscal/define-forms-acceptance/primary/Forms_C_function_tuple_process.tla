---- MODULE Forms_C_function_tuple_process ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[<<a,b>> \in {<<1,2>>}] == a+b
 }
process (P \in {1,2}) variable z=0; { L:y:=F[<<1,2>>]; E:skip; }
}
*)
====
