---- MODULE Context_P_macro ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 end define;
macro M(a) begin x := F(a); end macro;
begin L: M(1); end algorithm;
*)
====
