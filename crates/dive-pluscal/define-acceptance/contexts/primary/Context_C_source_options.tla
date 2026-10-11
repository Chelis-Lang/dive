---- MODULE Context_C_source_options ----
EXTENDS Naturals, Integers, Sequences, TLC
(* PlusCal options (-wf) *)
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F(a) == a + 1
 }
{ L: x := F(1); }
}
*)
====
