---- MODULE ProcedureCrc_C_with_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:x:=a; R:return;
};
{
L:with (q=2) { call Put(q); goto E; }; E:skip;
}
} *)
====
