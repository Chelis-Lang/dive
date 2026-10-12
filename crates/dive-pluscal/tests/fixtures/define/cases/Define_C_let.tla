---- MODULE Define_C_let ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == LET G == a+1 IN G
 }
{ L: x := F(2); }
}
*)
====
