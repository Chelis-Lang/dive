---- MODULE ProcessHeader_C_boundary_nonfinite_domain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker \in Nat)
{ L:call First(2); E:skip; }

} *)
====
