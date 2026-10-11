---- MODULE Procedure_C_two_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0,b=1)

{
P: x:=a+b; R: return;
};
{
L: call Put(2,3); E: skip;
}
} *)
====
