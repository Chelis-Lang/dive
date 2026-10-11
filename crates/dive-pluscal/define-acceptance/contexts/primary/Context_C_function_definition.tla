---- MODULE Context_C_function_definition ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F[a \in {1,2}] == a+1
 }
{ L: x := F[1]; }
}
*)
====
