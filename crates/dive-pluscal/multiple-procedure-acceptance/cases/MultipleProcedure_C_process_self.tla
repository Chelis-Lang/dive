---- MODULE MultipleProcedure_C_process_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in {1})

{ L:call First(self); E:skip; }
process (Two \in {2})

{ M:call First(self); N:skip; }
} *)
====
