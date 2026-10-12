---- MODULE ProcedureGraph_C_arity_low ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(); R:return;
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
