---- MODULE Recursive_C_self_two_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1,v=u+1;
{
F:if(a>0){ call First(a-1); }else{ x:=u+v; }; R:x:=x+u+v; return;
};
{
L:call First(2); E:skip;
}
} *)
====
