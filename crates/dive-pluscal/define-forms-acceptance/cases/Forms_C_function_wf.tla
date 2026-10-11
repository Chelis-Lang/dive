---- MODULE Forms_C_function_wf ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}] == a+1
 }
{ L:y:=F[1]; E:skip; }
}
*)
====
