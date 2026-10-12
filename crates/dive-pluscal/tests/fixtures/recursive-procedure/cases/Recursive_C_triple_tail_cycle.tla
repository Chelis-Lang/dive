---- MODULE Recursive_C_triple_tail_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call Second(a-1); return; }else{ x:=a; }; R:return;
};
procedure Second(b=0)

{
S:if(b>0){ call Third(b-1); return; }else{ x:=b; }; T:return;
};
procedure Third(c=0)

{
U:if(c>0){ call First(c-1); return; }else{ x:=c; }; V:return;
};
{
L:call First(4); E:skip;
}
} *)
====
