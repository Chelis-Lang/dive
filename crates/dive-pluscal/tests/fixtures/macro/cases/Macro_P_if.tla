---- MODULE Macro_P_if ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin if a>0 then v:=a; else v:=0; end if; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
