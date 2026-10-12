---- MODULE Macro_P_simultaneous ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(a, b) begin x := <<a,b>>; end macro; begin L: Put(b, 3); E: skip; end algorithm; *)
====
