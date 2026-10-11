---- MODULE Define_C_set ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == {a,1}
 }
{ L: x := F(2); }
}
*)
====
