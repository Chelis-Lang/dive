---- MODULE MultipleProcedure_C_earlier_process_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)
variable y=1,q=y+1;
{ L:call First(1); E:skip; }
process (Two \in {2})
variable z=self,t=z+1;
{ M:call First(2); N:skip; }
} *)
====
