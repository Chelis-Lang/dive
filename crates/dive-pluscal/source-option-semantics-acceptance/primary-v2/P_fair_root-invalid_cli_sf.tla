---- MODULE P_fair_root_invalid_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-labelRoot 9Bad)
(* --fair algorithm A variables g=0; process P\in {1,2} begin L:g:=1; E:skip; end process; end algorithm; *)
====
