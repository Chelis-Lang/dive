---- MODULE Context_P_source_options ----
EXTENDS Naturals, Integers, Sequences, TLC
(* PlusCal options (-wf) *)
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 end define;
begin L: x := F(1); end algorithm;
*)
====
