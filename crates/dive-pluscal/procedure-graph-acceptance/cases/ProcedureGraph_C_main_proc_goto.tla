---- MODULE ProcedureGraph_C_main_proc_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F: x:=a; R: return;
};
procedure Second(b=1)

{
S: x:=b; T: return;
};
{
L:goto F; E:skip;
}
} *)
====
