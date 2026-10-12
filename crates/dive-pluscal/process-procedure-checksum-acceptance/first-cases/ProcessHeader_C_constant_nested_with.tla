---- MODULE ProcessHeader_C_constant_nested_with ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker = 1)
{ L:with(t=2){call First(t);} E:skip; }

} *)
====
