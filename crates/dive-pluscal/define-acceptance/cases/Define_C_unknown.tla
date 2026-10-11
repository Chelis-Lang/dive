---- MODULE Define_C_unknown ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == Unknown
 }
{ L: x := F; }
}
*)
====
