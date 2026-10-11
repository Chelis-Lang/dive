---- MODULE Ownership_P_unclosed_quote ----
EXTENDS Integers
(* --algorithm A variable x=0; macro M(a) begin x:=a; end macro; begin L:M(1); E:skip; end algorithm; "abc *)
====
