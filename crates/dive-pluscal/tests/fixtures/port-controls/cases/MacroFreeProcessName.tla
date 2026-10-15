---- MODULE MacroFreeProcessName ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; macro M(v) begin v:=w; end macro; process P=1 variable z=0; begin L: M(z); end process; process Q=2 variable w=0; begin K: skip; end process; end algorithm
*)
====
