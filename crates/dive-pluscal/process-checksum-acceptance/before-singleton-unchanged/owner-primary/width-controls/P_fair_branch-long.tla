---- MODULE P_fair_branch_long ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0,abcdef=9; process P = 1 begin L:if g=0 then abcdef:=1; else g:=1; end if; E:skip; end process; end algorithm; *)
====
