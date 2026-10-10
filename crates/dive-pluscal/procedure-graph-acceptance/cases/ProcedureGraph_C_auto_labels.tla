---- MODULE ProcedureGraph_C_auto_labels ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
call Second(a); return;
};
procedure Second(b=1)

{
x:=b; return;
};
{
call First(2); skip;
}
} *)
====
