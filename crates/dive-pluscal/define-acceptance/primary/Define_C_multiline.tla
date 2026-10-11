---- MODULE Define_C_multiline ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) ==
  IF a > 0 THEN a ELSE 0
 }
{ L: x := F(1); }
}
*)
====
