---- MODULE Pc_P_unicode_expr ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A variable x ∈ {1, 2}; begin assert ∃ a ∈ {1, 2} : a = a; x := ⟨1, 2⟩; end algorithm

*)
====
