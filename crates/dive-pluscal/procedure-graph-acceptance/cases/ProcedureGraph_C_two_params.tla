---- MODULE ProcedureGraph_C_two_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=1)

{
F:call Second(a,c); R:return;
};
procedure Second(b=1,d=2)

{
S:x:=b+d; T:return;
};
{
L:call First(2,3); E:skip;
}
} *)
====
