---- MODULE Recursive_C_self_local_mutate ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=0;
{
F:u:=a+1; C:if(a>0){ call First(a-1); }else{ x:=u; }; R:x:=x+u; return;
};
{
L:call First(2); E:skip;
}
} *)
====
