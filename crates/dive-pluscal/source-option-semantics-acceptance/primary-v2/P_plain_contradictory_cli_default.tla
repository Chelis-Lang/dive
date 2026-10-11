---- MODULE P_plain_contradictory_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf, -sf)
(* --algorithm A variables g=0; process P\in {1,2} begin L:g:=1; E:skip; end process; end algorithm; *)
====
