---- MODULE MacroProcess_P_unused_foreign ----
EXTENDS Integers, TLC
(* --algorithm A variable x=0,y=0; macro Put(v,a) begin v:=a; end macro;macro Unused() begin z:=w; end macro; process P\in {1,2} variable z=0; begin L: Put(z,self); E: skip; end process;process Q\in {3,4} variable w=0; begin M: Put(w,self); F: skip; end process; end algorithm; *)
====
