---- MODULE Define_P_parameter_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 end define;
 process P \in {1,2}
 variables z=0;
 begin L: x := F(1); end process;
end algorithm;
*)
====
