---- MODULE Ownership_P_bad_token ----
EXTENDS Integers
(* --algorithm A variable x=0; macro M(a) begin x:=a; end macro; begin L:M(1); E:skip; end algorithm; ` *)
====
