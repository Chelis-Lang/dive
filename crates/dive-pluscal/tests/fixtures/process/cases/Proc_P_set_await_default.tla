---- MODULE Proc_P_set_await_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=0; begin L: await x=0; x:=1; E: skip; end process; end algorithm; *)
====
