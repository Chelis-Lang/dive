---- MODULE ProcedureGraph_C_reserved_wrapper ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F: x:=a; R: return;
};
procedure Next(b=1)

{
S:x:=b; T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
