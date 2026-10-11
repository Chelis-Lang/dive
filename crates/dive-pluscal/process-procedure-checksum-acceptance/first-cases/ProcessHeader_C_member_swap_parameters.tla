---- MODULE ProcessHeader_C_member_swap_parameters ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,b=0)

{ F:if(a>0){call First(a-1,a); return;}else{x:=b; goto R;} R:return; };
process (Worker \in {1,2})
{ L:call First(2,3); E:skip; }

} *)
====
