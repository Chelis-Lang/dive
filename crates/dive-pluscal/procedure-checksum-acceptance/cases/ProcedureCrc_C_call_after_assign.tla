---- MODULE ProcedureCrc_C_call_after_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:x:=a; R:return;
};
{
L:x:=1; call Put(x+1); E:skip;
}
} *)
====
