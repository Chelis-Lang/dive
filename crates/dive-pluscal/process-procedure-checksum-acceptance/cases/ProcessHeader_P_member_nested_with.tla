---- MODULE ProcessHeader_P_member_nested_with ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker \in {1,2}
begin
L:with t=2 do call First(t); end with; E:skip;
end process;

end algorithm; *)
====
