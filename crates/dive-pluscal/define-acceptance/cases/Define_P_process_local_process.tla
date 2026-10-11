---- MODULE Define_P_process_local_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == z
 end define;
 process P \in {1,2}
 variables z=0;
 begin L: x := F; end process;
end algorithm;
*)
====
