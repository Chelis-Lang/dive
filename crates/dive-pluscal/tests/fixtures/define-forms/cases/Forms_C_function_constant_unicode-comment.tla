---- MODULE Forms_C_function_constant_unicode_comment ----
EXTENDS Integers, Sequences, TLC, FiniteSets
(*
--algorithm A {
 variables x=0,y=0;
 define \* comment λ🙂
 {
 F[a \in {1,2}] == a+1
 }
{ L:y:=F[1]; E:skip; }
}
*)
====
