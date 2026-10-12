---- MODULE P_fair_if ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 begin L:if g=0 then g:=1; else g:=2; end if; E:skip; end process; end algorithm; *)
====
