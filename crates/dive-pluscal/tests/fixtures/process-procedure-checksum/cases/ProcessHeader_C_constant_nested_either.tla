---- MODULE ProcessHeader_C_constant_nested_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker = 1)
{ L:either{call First(2);}or{call First(3);}; E:skip; }

} *)
====
