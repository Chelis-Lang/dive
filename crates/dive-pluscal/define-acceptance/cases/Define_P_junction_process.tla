---- MODULE Define_P_junction_process ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == /\ x=0
     /\ y=0
 end define;
 process P \in {1,2}
 variables z=0;
 begin L: if F then x := 1; end if; end process;
end algorithm;
*)
====
