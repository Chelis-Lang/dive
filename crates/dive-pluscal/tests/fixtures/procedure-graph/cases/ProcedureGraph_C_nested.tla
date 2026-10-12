---- MODULE ProcedureGraph_C_nested ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a+1); R:x:=x+a; return;
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
