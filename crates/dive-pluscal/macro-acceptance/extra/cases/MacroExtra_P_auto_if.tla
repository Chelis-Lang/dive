---- MODULE MacroExtra_P_auto_if ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin if a>0 then v:=a; else v:=0; end if; end macro; begin Put(x, 1); skip; end algorithm; *)
====
