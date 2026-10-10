---- MODULE Recursive_C_mutual_tail_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1;
{
F:if(a>0){ call Second(a-1); return; }else{ x:=u; }; R:return;
};
procedure Second(b=0)
variable v=b+2;
{
S:if(b>0){ call First(b-1); return; }else{ x:=v; }; T:return;
};
{
L:call First(2); E:skip;
}
} *)
====
