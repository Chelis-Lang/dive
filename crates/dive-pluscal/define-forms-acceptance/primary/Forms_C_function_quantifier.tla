---- MODULE Forms_C_function_quantifier ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define {
 F[a \in {1,2}]==IF \E q \in {1,2}:q=a THEN a ELSE 0
 }
{ L:y:=F[1]; E:skip; }
}
*)
====
