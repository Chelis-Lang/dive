---- MODULE Forms_C_function_let ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}]==LET H(q)==q+1 IN H(a)
 }
{ L:y:=F[1]; E:skip; }
}
*)
====
