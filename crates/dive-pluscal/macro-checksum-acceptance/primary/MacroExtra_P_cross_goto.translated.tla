---- MODULE MacroExtra_P_cross_goto ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin goto F; end macro; process P=1 variable z=0; begin L: Put(z,1); E: skip; end process; process Q \in {2,3} variable w=0; begin M: Put(w,self); F: skip; end process; end algorithm; *)
====
