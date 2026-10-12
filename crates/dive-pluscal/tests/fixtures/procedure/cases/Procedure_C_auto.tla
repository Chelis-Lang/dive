---- MODULE Procedure_C_auto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
x:=a; return;
};
{
call Put(2); skip;
}
} *)
====
