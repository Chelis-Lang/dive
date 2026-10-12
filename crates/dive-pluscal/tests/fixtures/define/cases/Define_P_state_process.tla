---- MODULE Define_P_state_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == x
 end define;
 process P \in {1,2}
 variables z=0;
 begin L: x := 1; y := F; end process;
end algorithm;
*)
====
