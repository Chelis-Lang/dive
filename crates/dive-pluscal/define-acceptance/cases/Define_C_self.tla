---- MODULE Define_C_self ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == self
 }
{ L: x := F; }
}
*)
====
