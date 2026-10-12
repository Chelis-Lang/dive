---- MODULE Context_P_unicode_comment ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 (* outside λ *) F(a) == a + 1
 end define;
begin L: x := F(1); end algorithm;
*)
====
