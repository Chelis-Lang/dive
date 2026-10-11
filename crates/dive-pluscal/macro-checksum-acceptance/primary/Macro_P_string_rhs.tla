---- MODULE Macro_P_string_rhs ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(a) begin x := <<"a",a>>; end macro; begin L: Put(2); E: skip; end algorithm; *)
====
