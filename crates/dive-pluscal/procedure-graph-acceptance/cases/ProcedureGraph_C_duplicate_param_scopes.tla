---- MODULE ProcedureGraph_C_duplicate_param_scopes ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F: x:=a; R: return;
};
procedure Second(a=1)

{
S:x:=a; T:return;
};
{
L: call First(2); E: skip;
}
} *)
====
