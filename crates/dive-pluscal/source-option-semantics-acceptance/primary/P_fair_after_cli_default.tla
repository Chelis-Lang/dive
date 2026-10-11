---- MODULE P_fair_after_cli_default ----
EXTENDS Integers,TLC,Sequences

(* --fair algorithm A variables g=0; process P\in {1,2} begin L:g:=1; E:skip; end process; end algorithm; *)
\* PlusCal options (-wf)
====
