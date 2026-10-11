---- MODULE Define_C_with_shadow ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a+1
 }
{ L: with (F = 2) { x := F; }; }
}
*)
====
