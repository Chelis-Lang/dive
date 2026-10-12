---- MODULE Define_C_sequence ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == <<a,a>>
 }
{ L: x := F(2); }
}
*)
====
