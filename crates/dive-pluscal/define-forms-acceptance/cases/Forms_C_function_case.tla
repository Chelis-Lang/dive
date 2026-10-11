---- MODULE Forms_C_function_case ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}]==CASE a=1 -> 2 [] OTHER -> 0
 }
{ L:y:=F[1]; E:skip; }
}
*)
====
