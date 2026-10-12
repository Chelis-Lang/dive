---- MODULE ProcessHeader_P_member_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=1;
begin
F:x:=u+a; R:return;
end procedure;
process Worker \in {1,2}
begin
L:call First(2); E:skip;
end process;

end algorithm; *)
====
