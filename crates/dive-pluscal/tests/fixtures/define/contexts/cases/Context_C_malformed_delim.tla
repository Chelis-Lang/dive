---- MODULE Context_C_malformed_delim ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == (1]
 }
{ L: x := F(1); }
}
*)
====
