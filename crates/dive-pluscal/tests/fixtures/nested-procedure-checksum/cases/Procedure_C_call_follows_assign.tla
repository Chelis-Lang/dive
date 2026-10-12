---- MODULE Procedure_C_call_follows_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: x:=a; return;
};
{
L: x:=1; call Put(x+1); E: skip;
}
} *)
====
