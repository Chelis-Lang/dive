---- MODULE MacroProcess_P_strong ----
EXTENDS Integers, TLC
(* --algorithm A variable x=0,y=0; macro Put(v,a) begin v:=a; end macro; fair + process P\in {1,2} variable z=0; begin L: Put(z,self); E: skip; end process; end algorithm; *)
====
