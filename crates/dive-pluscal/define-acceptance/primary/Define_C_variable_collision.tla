---- MODULE Define_C_variable_collision ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 x == 1
 }
{ L: x := 2; }
}
*)
====
