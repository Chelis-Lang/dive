---- MODULE ProcedureCrc_C_default_param ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a)

{
P:x:=a; R:return;
};
{
L:call Put(2); E:skip;
}
} *)
====
