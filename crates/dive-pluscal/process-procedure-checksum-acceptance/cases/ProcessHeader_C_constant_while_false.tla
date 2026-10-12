---- MODULE ProcessHeader_C_constant_while_false ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker = 1)
{ L:while(FALSE){call First(2);}; E:skip; }

} *)
====
