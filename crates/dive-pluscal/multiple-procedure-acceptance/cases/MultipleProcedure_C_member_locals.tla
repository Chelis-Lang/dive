---- MODULE MultipleProcedure_C_member_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in {1})
variable y=self;
{ L:call First(y); E:y:=x; }
process (Two \in {2})
variable z=self;
{ M:call First(z); N:z:=x; }
} *)
====
