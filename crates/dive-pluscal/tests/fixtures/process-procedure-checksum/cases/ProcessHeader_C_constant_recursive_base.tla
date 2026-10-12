---- MODULE ProcessHeader_C_constant_recursive_base ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:if(a>0){call First(a-1); goto R;}else{x:=a; goto R;}; R:return; };
process (Worker = 1)
{ L:call First(2); E:skip; }

} *)
====
