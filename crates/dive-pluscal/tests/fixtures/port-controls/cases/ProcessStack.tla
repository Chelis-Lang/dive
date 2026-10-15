---- MODULE ProcessStack ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; procedure P(a=0) begin F:x:=Len(stack[self]); R:return; end procedure; process Worker=1 begin L:call P(1); E:skip; end process; end algorithm
*)
====
