---- MODULE MultipleProcedure_C_recursive_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a;
{ F:if(a>0){ call First(a-1); }else{ x:=x+u; }; R:x:=x+u; return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
