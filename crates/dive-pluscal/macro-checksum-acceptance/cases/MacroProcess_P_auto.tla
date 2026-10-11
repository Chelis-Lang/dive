---- MODULE MacroProcess_P_auto ----
EXTENDS Integers, TLC
(* --algorithm A variable x=0,y=0; macro Put(v,a) begin v:=a; end macro; process P\in {1,2} variable z=0; begin Put(z,self); skip; end process; end algorithm; *)
====
