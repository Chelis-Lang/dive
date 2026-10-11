---- MODULE P_unterminated ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf
(* --algorithm A variables g=0; process P=1 begin L:g:=1; E:skip; end process; end algorithm; *)
====
