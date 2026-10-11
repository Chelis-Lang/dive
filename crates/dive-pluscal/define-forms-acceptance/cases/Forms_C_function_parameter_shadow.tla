---- MODULE Forms_C_function_parameter_shadow ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[x \in {1,2}]==x+1
 }
{ L:y:=F[1]; E:skip; }
}
*)
====
