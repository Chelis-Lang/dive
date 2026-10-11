---- MODULE Procedure_C_call_missing_follow_label ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: return;
};
{
L: call Put(2); x:=4; E: skip;
}
} *)
====
