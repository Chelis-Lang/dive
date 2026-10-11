---- MODULE Define_C_quantifier ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == \A i \in 1..a : i>0
 }
{ L: x := F(2); }
}
*)
====
