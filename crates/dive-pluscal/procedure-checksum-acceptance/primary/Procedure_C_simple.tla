---- MODULE Procedure_C_simple ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: x:=a; R: return;
};
{
L: call Put(2); E: skip;
}
} *)
====
