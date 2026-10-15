---- MODULE UnicodeForall ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; begin L: x:=1; print ∀ k ∈ {1}: k=x; end algorithm
*)
====
