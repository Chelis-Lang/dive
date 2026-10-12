---- MODULE ProcessHeader_P_member_two_parameters ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,b=0)

begin
F:x:=a+b; R:return;
end procedure;
process Worker \in {1,2}
begin
L:call First(2,3); E:skip;
end process;

end algorithm; *)
====
