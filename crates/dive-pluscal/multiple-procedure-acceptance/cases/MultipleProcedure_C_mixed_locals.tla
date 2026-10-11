---- MODULE MultipleProcedure_C_mixed_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in {1})
variable y=self;
{ L:call First(y); E:y:=x; }
process (Two = 2)
variable z=2;
{ M:call First(z); N:z:=x; }
} *)
====
