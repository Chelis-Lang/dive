---- MODULE Define_C_recursive ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == IF a=0 THEN 0 ELSE F(a-1)
 }
{ L: x := F(2); }
}
*)
====
