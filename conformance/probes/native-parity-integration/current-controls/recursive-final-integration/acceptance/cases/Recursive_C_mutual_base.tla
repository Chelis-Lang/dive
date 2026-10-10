---- MODULE Recursive_C_mutual_base ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call Second(a-1); }else{ x:=0; }; R:x:=x+a; return;
};
procedure Second(b=0)

{
S:if(b>0){ call First(b-1); }else{ x:=1; }; T:x:=x+b; return;
};
{
L:call First(2); E:skip;
}
} *)
====
