---- MODULE ProcedureCrc_C_unknown_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:x:=a; R:return;
};
{
L:call Missing(2); E:skip;
}
} *)
====
