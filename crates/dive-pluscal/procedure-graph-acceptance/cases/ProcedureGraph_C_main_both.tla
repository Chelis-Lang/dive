---- MODULE ProcedureGraph_C_main_both ----
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
L:call First(2); E:call Second(3); Z:skip;
}
} *)
====
