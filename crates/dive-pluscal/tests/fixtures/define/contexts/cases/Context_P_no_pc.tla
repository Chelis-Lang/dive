---- MODULE Context_P_no_pc ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 end define;
begin L: while TRUE do x := F(1); end while; end algorithm;
*)
====
