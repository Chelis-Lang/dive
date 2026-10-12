---- MODULE P_fair_elsif ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 begin L:if g=0 then g:=1; elsif g=1 then g:=2; else g:=3; end if; E:skip; end process; end algorithm; *)
====
