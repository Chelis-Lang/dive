---- MODULE Context_C_recursive_decl ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 RECURSIVE F(_)
 F(a) == IF a=0 THEN 0 ELSE F(a-1)
 }
{ L: x := F(1); }
}
*)
====
