---- MODULE ProcedureGraph_C_return_transfer_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:a:=a+1; return;
};
procedure Second(b=1)

{
S: x:=b; T: return;
};
{
L: call First(2); E: skip;
}
} *)
====
