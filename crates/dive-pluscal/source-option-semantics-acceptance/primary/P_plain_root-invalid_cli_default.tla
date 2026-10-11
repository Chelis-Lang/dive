---- MODULE P_plain_root_invalid_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-labelRoot 9Bad)
(* --algorithm A variables g=0; process P\in {1,2} begin L:g:=1; E:skip; end process; end algorithm; *)
====
