---- MODULE ProcedureGraph_C_duplicate_proc ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F: x:=a; R: return;
};
procedure First(b=0)

{
S:return;
};
{
L: call First(2); E: skip;
}
} *)
====
