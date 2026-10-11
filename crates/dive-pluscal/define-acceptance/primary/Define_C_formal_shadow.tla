---- MODULE Define_C_formal_shadow ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(x) == x+1
 }
{ L: x := F(2); }
}
*)
====
