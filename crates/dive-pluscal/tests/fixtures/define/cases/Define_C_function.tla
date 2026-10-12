---- MODULE Define_C_function ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == [i \in 1..a |-> i]
 }
{ L: x := F(2); }
}
*)
====
