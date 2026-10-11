---- MODULE P_plain_missing_close_cli_sf ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf
(* --algorithm A variables g=0; process P\in {1,2} begin L:g:=1; E:skip; end process; end algorithm; *)
====
