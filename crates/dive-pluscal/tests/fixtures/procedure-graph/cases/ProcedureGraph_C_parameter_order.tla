---- MODULE ProcedureGraph_C_parameter_order ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=a+1)

{
F:call Second(a,c); R:return;
};
procedure Second(b=1,d=b+1)

{
S:x:=b+d; T:return;
};
{
L:call First(2,3); E:skip;
}
} *)
====
