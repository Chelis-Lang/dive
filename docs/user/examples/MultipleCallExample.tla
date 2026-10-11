---- MODULE MultipleCallExample ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Add(a = 0)
begin
  Entry: x := x + a;
  Return: return;
end procedure;
process One = 1
begin
  First: call Add(1);
  FinishOne: skip;
end process;
process Two \in {2}
begin
  Second: call Add(self);
  FinishTwo: skip;
end process;
end algorithm; *)
====
