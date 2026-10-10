---- MODULE ProcedureGraph_C_callee_name_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F: x:=a; R: return;
};
procedure Second(First=1)

{
S:return;
};
{
L: call First(2); E: skip;
}
} *)
====
