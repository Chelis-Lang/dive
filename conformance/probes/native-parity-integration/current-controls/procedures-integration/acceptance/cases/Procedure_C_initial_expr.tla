---- MODULE Procedure_C_initial_expr ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=4)
variable b=a+1;
{
P: x:=b; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
