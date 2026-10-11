---- MODULE ProcedureCrc_C_wrong_arity ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:x:=a; R:return;
};
{
L:call Put(); E:skip;
}
} *)
====
