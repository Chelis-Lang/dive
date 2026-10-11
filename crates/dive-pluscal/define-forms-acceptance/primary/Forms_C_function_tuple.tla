---- MODULE Forms_C_function_tuple ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[<<a,b>> \in {<<1,2>>}] == a+b
 }
{ L:y:=F[<<1,2>>]; E:skip; }
}
*)
====
