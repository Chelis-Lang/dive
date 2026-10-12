---- MODULE P_fair_if_no_else ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 begin L:if g=0 then g:=1; end if; E:skip; end process; end algorithm; *)
====
