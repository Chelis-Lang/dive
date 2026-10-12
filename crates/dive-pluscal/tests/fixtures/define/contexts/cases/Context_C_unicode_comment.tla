---- MODULE Context_C_unicode_comment ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 (* outside λ *) F(a) == a + 1
 }
{ L: x := F(1); }
}
*)
====
