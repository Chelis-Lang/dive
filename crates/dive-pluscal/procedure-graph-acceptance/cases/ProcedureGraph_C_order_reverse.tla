---- MODULE ProcedureGraph_C_order_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Second(b=1)

{
S: x:=b; T: return;
};
procedure First(a=0)

{
F: x:=a; R: return;
};
{
L: call First(2); E: skip;
}
} *)
====
