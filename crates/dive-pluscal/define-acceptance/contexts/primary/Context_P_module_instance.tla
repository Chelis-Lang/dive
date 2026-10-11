---- MODULE Context_P_module_instance ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 I == INSTANCE Naturals
 end define;
begin L: x := F(1); end algorithm;
*)
====
