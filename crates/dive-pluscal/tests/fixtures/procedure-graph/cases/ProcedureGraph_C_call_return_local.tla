---- MODULE ProcedureGraph_C_call_return_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable c=a+1;
{
F:call Second(c); return;
};
procedure Second(b=1)
variable d=b+2;
{
S:x:=d; T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
