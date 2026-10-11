---- MODULE MacroExtra_P_multiple_fair ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro; fair + process P=1 variable z=0; begin L: + Put(z,1); E: skip; end process; fair process Q \in {2,3} variable w=0; begin M: - Put(w,self); F: skip; end process; end algorithm; *)
====
