---- MODULE Procedure_C_local_read_main ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable b=1;
{
P: x:=a; return;
};
{
L: call Put(2); E: x:=b;
}
} *)
====
