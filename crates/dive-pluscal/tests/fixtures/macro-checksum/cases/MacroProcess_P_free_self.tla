---- MODULE MacroProcess_P_free_self ----
EXTENDS Integers, TLC
(* --algorithm A variable x=0,y=0; macro Put(v,a) begin v:=self; end macro; process P=1 variable z=0; begin L: Put(z,self); E: skip; end process; end algorithm; *)
====
