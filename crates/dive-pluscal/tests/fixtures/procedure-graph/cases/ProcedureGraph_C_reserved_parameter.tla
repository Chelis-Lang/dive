---- MODULE ProcedureGraph_C_reserved_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F: x:=a; R: return;
};
procedure Second(Next=1)

{
S:x:=Next; T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
