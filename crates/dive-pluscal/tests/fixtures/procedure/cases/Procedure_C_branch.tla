---- MODULE Procedure_C_branch ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: if(a>0){ x:=a; }else{x:=1;}; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
