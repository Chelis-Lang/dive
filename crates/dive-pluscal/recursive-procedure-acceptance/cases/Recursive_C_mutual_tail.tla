---- MODULE Recursive_C_mutual_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call Second(a-1); return; }else{ x:=a; }; R:return;
};
procedure Second(b=0)

{
S:if(b>0){ call First(b-1); return; }else{ x:=b; }; T:return;
};
{
L:call First(2); E:skip;
}
} *)
====
