---- MODULE OverlappingDomains ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; procedure P(a=0) begin F:x:=a; R:return; end procedure; process One=1 begin L:call P(1); E:skip; end process; process Two=1 begin M:call P(2); N:skip; end process; end algorithm
*)
====
