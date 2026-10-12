---- MODULE ProcedureGraph_C_branch_nested ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call Second(a); }else{x:=1;}; R:return;
};
procedure Second(b=1)

{
S: x:=b; T: return;
};
{
L: call First(2); E: skip;
}
} *)
====
