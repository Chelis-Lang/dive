---- MODULE P_plain_unrelated_first_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal ignored
\* PlusCal options (-wf)
(* --algorithm A variables g=0; process P\in {1,2} begin L:g:=1; E:skip; end process; end algorithm; *)
====
