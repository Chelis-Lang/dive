---- MODULE MultipleProcedure_C_infinite_call_loop ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)

{ L:while(TRUE){ call First(1); }; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
