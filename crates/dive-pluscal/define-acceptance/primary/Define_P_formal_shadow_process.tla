---- MODULE Define_P_formal_shadow_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(x) == x+1
 end define;
 process P \in {1,2}
 variables z=0;
 begin L: x := F(2); end process;
end algorithm;
*)
====
