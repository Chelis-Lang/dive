---- MODULE Define_C_duplicate ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == 1
 F == 2
 }
{ L: x := F; }
}
*)
====
