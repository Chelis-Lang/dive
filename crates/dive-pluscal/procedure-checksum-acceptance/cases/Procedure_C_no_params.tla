---- MODULE Procedure_C_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put()

{
P: x:=1; R: return;
};
{
L: call Put(); E: skip;
}
} *)
====
