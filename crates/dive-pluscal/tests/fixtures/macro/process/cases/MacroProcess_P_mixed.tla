---- MODULE MacroProcess_P_mixed ----
EXTENDS Integers, TLC
(* --algorithm A variable x=0,y=0; macro Put(v,a) begin v:=a; end macro; process P=1 variable z=0; begin L: Put(z,self); E: skip; end process;process Q\in {2,3} variable w=0; begin M: Put(w,self); F: skip; end process; end algorithm; *)
====
