---- MODULE Define_C_escaped ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == "a\n"
 }
{ L: x := F; }
}
*)
====
